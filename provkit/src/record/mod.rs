//! Records: the eighteen PROV-DM types and the enum that holds them.

use crate::well_known::PROV;
use crate::{Attributes, DateTime, Error, QName, Result};

/// A formal attribute name (PROV-DM's positional arguments).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FormalAttr {
    /// `prov:entity`.
    Entity,
    /// `prov:activity`.
    Activity,
    /// `prov:agent`.
    Agent,
    /// `prov:plan`.
    Plan,
    /// `prov:trigger`.
    Trigger,
    /// `prov:starter`.
    Starter,
    /// `prov:ender`.
    Ender,
    /// `prov:informed`.
    Informed,
    /// `prov:informant`.
    Informant,
    /// `prov:delegate`.
    Delegate,
    /// `prov:responsible`.
    Responsible,
    /// `prov:influencee`.
    Influencee,
    /// `prov:influencer`.
    Influencer,
    /// `prov:generatedEntity`.
    GeneratedEntity,
    /// `prov:usedEntity`.
    UsedEntity,
    /// `prov:generation`.
    Generation,
    /// `prov:usage`.
    Usage,
    /// `prov:specificEntity`.
    SpecificEntity,
    /// `prov:generalEntity`.
    GeneralEntity,
    /// `prov:alternate1`.
    Alternate1,
    /// `prov:alternate2`.
    Alternate2,
    /// `prov:collection`.
    Collection,
    /// `prov:bundle`.
    Bundle,
    /// `prov:time`.
    Time,
    /// `prov:startTime`.
    StartTime,
    /// `prov:endTime`.
    EndTime,
}

use FormalAttr as F;

impl FormalAttr {
    /// Every formal attribute.
    pub const ALL: &'static [FormalAttr] = &[
        F::Entity,
        F::Activity,
        F::Agent,
        F::Plan,
        F::Trigger,
        F::Starter,
        F::Ender,
        F::Informed,
        F::Informant,
        F::Delegate,
        F::Responsible,
        F::Influencee,
        F::Influencer,
        F::GeneratedEntity,
        F::UsedEntity,
        F::Generation,
        F::Usage,
        F::SpecificEntity,
        F::GeneralEntity,
        F::Alternate1,
        F::Alternate2,
        F::Collection,
        F::Bundle,
        F::Time,
        F::StartTime,
        F::EndTime,
    ];

    /// The local name under `prov:` (`entity`, `generatedEntity`, `startTime`, ...).
    pub fn local_name(self) -> &'static str {
        match self {
            F::Entity => "entity",
            F::Activity => "activity",
            F::Agent => "agent",
            F::Plan => "plan",
            F::Trigger => "trigger",
            F::Starter => "starter",
            F::Ender => "ender",
            F::Informed => "informed",
            F::Informant => "informant",
            F::Delegate => "delegate",
            F::Responsible => "responsible",
            F::Influencee => "influencee",
            F::Influencer => "influencer",
            F::GeneratedEntity => "generatedEntity",
            F::UsedEntity => "usedEntity",
            F::Generation => "generation",
            F::Usage => "usage",
            F::SpecificEntity => "specificEntity",
            F::GeneralEntity => "generalEntity",
            F::Alternate1 => "alternate1",
            F::Alternate2 => "alternate2",
            F::Collection => "collection",
            F::Bundle => "bundle",
            F::Time => "time",
            F::StartTime => "startTime",
            F::EndTime => "endTime",
        }
    }

    /// The formal for a `prov:` local name.
    pub fn from_local_name(name: &str) -> Option<FormalAttr> {
        FormalAttr::ALL
            .iter()
            .copied()
            .find(|f| f.local_name() == name)
    }

    /// Whether the value is a dateTime rather than a name.
    pub fn is_time(self) -> bool {
        matches!(self, F::Time | F::StartTime | F::EndTime)
    }
}

/// An owned formal attribute value, for [`Record::from_parts`].
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Formal {
    /// A name-valued formal.
    Name(QName),
    /// A time-valued formal.
    Time(DateTime),
}

/// A borrowed formal attribute value, from [`Record::formals`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FormalRef<'a> {
    /// A name-valued formal.
    Name(&'a QName),
    /// A time-valued formal.
    Time(&'a DateTime),
}

impl FormalRef<'_> {
    /// An owned copy.
    pub fn to_owned(&self) -> Formal {
        match *self {
            FormalRef::Name(q) => Formal::Name(q.clone()),
            FormalRef::Time(t) => Formal::Time(t.clone()),
        }
    }
}

/// A type that a formal's field holds, which is [`QName`] for a name and
/// [`DateTime`] for a time.
trait FormalValue {
    /// The value that `formal` holds. [`Formals::new`] has checked its type.
    fn from_formal(formal: Formal) -> Self;

    /// The value as a borrowed formal.
    fn as_formal(&self) -> FormalRef<'_>;
}

impl FormalValue for QName {
    fn from_formal(formal: Formal) -> Self {
        match formal {
            Formal::Name(q) => q,
            Formal::Time(_) => unreachable!("checked in Formals::new"),
        }
    }

    fn as_formal(&self) -> FormalRef<'_> {
        FormalRef::Name(self)
    }
}

impl FormalValue for DateTime {
    fn from_formal(formal: Formal) -> Self {
        match formal {
            Formal::Time(t) => t,
            Formal::Name(_) => unreachable!("checked in Formals::new"),
        }
    }

    fn as_formal(&self) -> FormalRef<'_> {
        FormalRef::Time(self)
    }
}

/// Generates [`Kind`], [`Record`], the struct of each kind and every `match`
/// over the kinds from one table.
///
/// A row is `Variant "keyword" shape [arities] { field: FormalAttr, .. }`,
/// under the struct's doc line. The shape is `element` or `relation`, and the
/// arities are the argument counts PROV-N allows after the identifier. Each
/// field carries its own doc line.
macro_rules! records {
    // The identifier is mandatory on an element and optional on a relation.
    (@is_element element) => { true };
    (@is_element relation) => { false };
    (@id_type element) => { QName };
    (@id_type relation) => { Option<QName> };
    (@new_id element $kind:ident $id:ident) => {
        $id.ok_or_else(|| invalid($kind, "needs an identifier".to_owned()))?
    };
    (@new_id relation $kind:ident $id:ident) => { $id };
    (@id element $record:ident) => { Some(&$record.id) };
    (@id relation $record:ident) => { $record.id.as_ref() };

    // A formal that `FormalAttr::is_time` names holds a time; the rest hold names.
    (@value_type Time) => { DateTime };
    (@value_type StartTime) => { DateTime };
    (@value_type EndTime) => { DateTime };
    (@value_type $attr:ident) => { QName };

    ($(
        $(#[$doc:meta])*
        $name:ident $keyword:literal $shape:ident [$($arity:literal),*] {
            $($(#[$field_doc:meta])* $field:ident: $attr:ident,)*
        }
    )*) => {
        /// The PROV-DM type of a record.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        #[non_exhaustive]
        pub enum Kind {
            $(#[doc = concat!("`", $keyword, "`.")] $name,)*
        }

        impl Kind {
            /// Every kind, in PROV-DM order.
            pub const ALL: &'static [Kind] = &[$(Kind::$name),*];

            /// The kind's name, which is its PROV-N keyword and its PROV-JSON key.
            pub fn name(self) -> &'static str {
                match self {
                    $(Kind::$name => $keyword,)*
                }
            }

            /// The kind with that name.
            pub fn from_name(name: &str) -> Option<Kind> {
                match name {
                    $($keyword => Some(Kind::$name),)*
                    _ => None,
                }
            }

            /// Entity, activity and agent; everything else is a relation.
            pub fn is_element(self) -> bool {
                match self {
                    $(Kind::$name => records!(@is_element $shape),)*
                }
            }

            /// The kind's formal attributes, in PROV-DM order.
            pub fn formals(self) -> &'static [FormalAttr] {
                match self {
                    $(Kind::$name => &[$(FormalAttr::$attr),*],)*
                }
            }

            /// Argument counts PROV-N allows after the identifier.
            #[cfg(feature = "provn")]
            pub(crate) fn arities(self) -> &'static [usize] {
                match self {
                    $(Kind::$name => &[$($arity),*],)*
                }
            }
        }

        $(
            $(#[$doc])*
            #[derive(Clone, Debug, PartialEq, Eq, Hash)]
            pub struct $name {
                /// The identifier.
                pub id: records!(@id_type $shape),
                $($(#[$field_doc])* pub $field: Option<records!(@value_type $attr)>,)*
                /// Non-formal attributes.
                pub attributes: Attributes,
            }
        )*

        /// A PROV record of any of the eighteen PROV-DM types.
        #[derive(Clone, Debug, PartialEq, Eq, Hash)]
        #[non_exhaustive]
        pub enum Record {
            $($(#[$doc])* $name($name),)*
        }

        impl Record {
            /// The record's kind.
            pub fn kind(&self) -> Kind {
                match self {
                    $(Record::$name(_) => Kind::$name,)*
                }
            }

            /// The identifier, if any.
            pub fn id(&self) -> Option<&QName> {
                match self {
                    $(Record::$name(r) => records!(@id $shape r),)*
                }
            }

            /// Non-formal attributes.
            pub fn attributes(&self) -> &Attributes {
                match self {
                    $(Record::$name(r) => &r.attributes,)*
                }
            }

            /// Non-formal attributes, mutably.
            pub fn attributes_mut(&mut self) -> &mut Attributes {
                match self {
                    $(Record::$name(r) => &mut r.attributes,)*
                }
            }

            /// The value of one formal attribute, if present.
            pub fn formal(&self, attr: FormalAttr) -> Option<FormalRef<'_>> {
                match (self, attr) {
                    $($(
                        (Record::$name(r), FormalAttr::$attr) => {
                            r.$field.as_ref().map(FormalValue::as_formal)
                        }
                    )*)*
                    _ => None,
                }
            }

            /// A record of `kind` from parts that [`Record::from_parts`] has
            /// checked, apart from the identifier of an element.
            fn assemble(
                kind: Kind,
                identifier: Option<QName>,
                mut formals: Formals,
                attributes: Attributes,
            ) -> Result<Record> {
                Ok(match kind {
                    $(Kind::$name => Record::$name($name {
                        id: records!(@new_id $shape kind identifier),
                        $($field: formals.take(FormalAttr::$attr),)*
                        attributes,
                    }),)*
                })
            }
        }
    };
}

records! {
    /// An entity.
    Entity "entity" element [0] {}

    /// An activity.
    Activity "activity" element [0, 2] {
        /// `prov:startTime`.
        start_time: StartTime,
        /// `prov:endTime`.
        end_time: EndTime,
    }

    /// An agent.
    Agent "agent" element [0] {}

    /// A generation (`wasGeneratedBy`).
    Generation "wasGeneratedBy" relation [1, 3] {
        /// `prov:entity`.
        entity: Entity,
        /// `prov:activity`.
        activity: Activity,
        /// `prov:time`.
        time: Time,
    }

    /// A usage (`used`).
    Usage "used" relation [1, 3] {
        /// `prov:activity`.
        activity: Activity,
        /// `prov:entity`.
        entity: Entity,
        /// `prov:time`.
        time: Time,
    }

    /// A communication (`wasInformedBy`).
    Communication "wasInformedBy" relation [2] {
        /// `prov:informed`.
        informed: Informed,
        /// `prov:informant`.
        informant: Informant,
    }

    /// A start (`wasStartedBy`).
    Start "wasStartedBy" relation [1, 4] {
        /// `prov:activity`.
        activity: Activity,
        /// `prov:trigger`.
        trigger: Trigger,
        /// `prov:starter`.
        starter: Starter,
        /// `prov:time`.
        time: Time,
    }

    /// An end (`wasEndedBy`).
    End "wasEndedBy" relation [1, 4] {
        /// `prov:activity`.
        activity: Activity,
        /// `prov:trigger`.
        trigger: Trigger,
        /// `prov:ender`.
        ender: Ender,
        /// `prov:time`.
        time: Time,
    }

    /// An invalidation (`wasInvalidatedBy`).
    Invalidation "wasInvalidatedBy" relation [1, 3] {
        /// `prov:entity`.
        entity: Entity,
        /// `prov:activity`.
        activity: Activity,
        /// `prov:time`.
        time: Time,
    }

    /// A derivation (`wasDerivedFrom`).
    Derivation "wasDerivedFrom" relation [2, 5] {
        /// `prov:generatedEntity`.
        generated_entity: GeneratedEntity,
        /// `prov:usedEntity`.
        used_entity: UsedEntity,
        /// `prov:activity`.
        activity: Activity,
        /// `prov:generation`.
        generation: Generation,
        /// `prov:usage`.
        usage: Usage,
    }

    /// An attribution (`wasAttributedTo`).
    Attribution "wasAttributedTo" relation [2] {
        /// `prov:entity`.
        entity: Entity,
        /// `prov:agent`.
        agent: Agent,
    }

    /// An association (`wasAssociatedWith`).
    Association "wasAssociatedWith" relation [1, 3] {
        /// `prov:activity`.
        activity: Activity,
        /// `prov:agent`.
        agent: Agent,
        /// `prov:plan`.
        plan: Plan,
    }

    /// A delegation (`actedOnBehalfOf`).
    Delegation "actedOnBehalfOf" relation [2, 3] {
        /// `prov:delegate`.
        delegate: Delegate,
        /// `prov:responsible`.
        responsible: Responsible,
        /// `prov:activity`.
        activity: Activity,
    }

    /// An influence (`wasInfluencedBy`).
    Influence "wasInfluencedBy" relation [2] {
        /// `prov:influencee`.
        influencee: Influencee,
        /// `prov:influencer`.
        influencer: Influencer,
    }

    /// A specialisation (`specializationOf`).
    Specialization "specializationOf" relation [2] {
        /// `prov:specificEntity`.
        specific_entity: SpecificEntity,
        /// `prov:generalEntity`.
        general_entity: GeneralEntity,
    }

    /// An alternate (`alternateOf`).
    Alternate "alternateOf" relation [2] {
        /// `prov:alternate1`.
        alternate1: Alternate1,
        /// `prov:alternate2`.
        alternate2: Alternate2,
    }

    /// A mention (`mentionOf`).
    Mention "mentionOf" relation [3] {
        /// `prov:specificEntity`.
        specific_entity: SpecificEntity,
        /// `prov:generalEntity`.
        general_entity: GeneralEntity,
        /// `prov:bundle`.
        bundle: Bundle,
    }

    /// A membership (`hadMember`).
    Membership "hadMember" relation [2] {
        /// `prov:collection`.
        collection: Collection,
        /// `prov:entity`.
        entity: Entity,
    }
}

impl Kind {
    /// An error if `attributes` holds a name that spells one of this kind's
    /// formals (`prov:activity` on `wasGeneratedBy`).
    fn check_attributes(self, attributes: &Attributes) -> Result<()> {
        let own_formal = attributes
            .iter()
            .find_map(|(name, _)| formal_named(self, name));
        match own_formal {
            Some(attr) => Err(invalid(
                self,
                format!(
                    "prov:{} is given as an attribute; its value belongs in the positional argument",
                    attr.local_name()
                ),
            )),
            None => Ok(()),
        }
    }
}

impl Record {
    /// Build a record of `kind` from its parts. Elements need an identifier;
    /// formals must belong to the kind, have the right value type, and appear
    /// at most once. No attribute may be named `prov:` plus the local name of
    /// one of the kind's formals (`prov:activity` on `wasGeneratedBy`).
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidRecord`] naming `kind` when the parts break
    /// one of these rules.
    pub fn from_parts(
        kind: Kind,
        id: Option<QName>,
        formals: Vec<(FormalAttr, Formal)>,
        attributes: Attributes,
    ) -> Result<Record> {
        let formals = Formals::new(kind, formals)?;
        kind.check_attributes(&attributes)?;
        Record::assemble(kind, id, formals, attributes)
    }

    /// The formal attributes that are present, in table order.
    pub fn formals(&self) -> Vec<(FormalAttr, FormalRef<'_>)> {
        let present = |&attr: &FormalAttr| Some((attr, self.formal(attr)?));
        self.kind().formals().iter().filter_map(present).collect()
    }

    /// An error if an attribute is named after one of this record's own formals.
    #[cfg(any(feature = "json", feature = "provn"))]
    pub(crate) fn check_attributes(&self) -> Result<()> {
        self.kind().check_attributes(self.attributes())
    }
}

/// The formal of `kind` that `name` spells, when `name` is the IRI of one of
/// the kind's formals (the PROV namespace plus the formal's local name).
pub(crate) fn formal_named(kind: Kind, name: &QName) -> Option<FormalAttr> {
    kind.formals()
        .iter()
        .copied()
        .find(|f| name.is(PROV, f.local_name()))
}

fn invalid(kind: Kind, message: String) -> Error {
    Error::InvalidRecord { kind, message }
}

/// Consumes a list of formals, checking each against the kind's table.
struct Formals {
    items: Vec<(FormalAttr, Formal)>,
}

impl Formals {
    fn new(kind: Kind, items: Vec<(FormalAttr, Formal)>) -> Result<Self> {
        let allowed = kind.formals();
        for (i, (attr, value)) in items.iter().enumerate() {
            if !allowed.contains(attr) {
                return Err(invalid(
                    kind,
                    format!("no formal attribute prov:{}", attr.local_name()),
                ));
            }
            let time_expected = attr.is_time();
            if time_expected != matches!(value, Formal::Time(_)) {
                return Err(invalid(
                    kind,
                    format!(
                        "prov:{} needs a {}",
                        attr.local_name(),
                        if time_expected { "dateTime" } else { "name" }
                    ),
                ));
            }
            if items[..i].iter().any(|(a, _)| a == attr) {
                return Err(invalid(
                    kind,
                    format!("prov:{} given twice", attr.local_name()),
                ));
            }
        }
        Ok(Formals { items })
    }

    /// The value of `attr`, removed from the list.
    fn take<T: FormalValue>(&mut self, attr: FormalAttr) -> Option<T> {
        let i = self.items.iter().position(|(a, _)| *a == attr)?;
        Some(T::from_formal(self.items.remove(i).1))
    }
}

#[cfg(test)]
mod tests;
