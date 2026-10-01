//! Records: the eighteen PROV-DM types and the enum that holds them.

#[macro_use]
mod formal;

pub use formal::{Formal, FormalAttr, FormalRef};

use crate::well_known::PROV;
use crate::{Attributes, DateTime, Error, QName, Result};
use formal::{FormalValue, Formals};

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

    /// The formal attributes that are present, in the order of
    /// [`Kind::formals`].
    pub fn formals(&self) -> Vec<(FormalAttr, FormalRef<'_>)> {
        let present = |&attr: &FormalAttr| Some((attr, self.formal(attr)?));
        self.kind().formals().iter().filter_map(present).collect()
    }

    /// An error if an attribute is named after one of this record's own formals.
    pub(crate) fn check_attributes(&self) -> Result<()> {
        self.kind().check_attributes(self.attributes())
    }
}

/// Generates [`Kind`], [`Record`], the struct of each kind and every `match`
/// over the kinds from one table.
///
/// A row is `Variant "keyword" shape [arities] { field: FormalAttr, .. }`,
/// under the struct's doc line. The shape is `element` or `relation`, and
/// `relation bare` marks a relation PROV-DM gives no identifier. The arities
/// are the argument counts PROV-N allows after the identifier. Each field's
/// type and doc line come from the `formals!` table.
macro_rules! records {
    // The identifier is mandatory on an element and optional on a relation.
    (@is_element element) => { true };
    (@is_element relation) => { false };
    (@bare bare) => { true };
    (@bare) => { false };
    (@id_type element) => { QName };
    (@id_type relation) => { Option<QName> };
    (@new_id element $kind:ident $id:ident) => {
        $id.ok_or_else(|| invalid($kind, "needs an identifier".to_owned()))?
    };
    (@new_id relation $kind:ident $id:ident) => { $id };
    (@id element $record:ident) => { Some(&$record.id) };
    (@id relation $record:ident) => { $record.id.as_ref() };

    ($(
        $(#[$doc:meta])*
        $name:ident $keyword:literal $shape:ident $($bare:ident)? [$($arity:literal),*] {
            $($field:ident: $attr:ident,)*
        }
    )*) => {
        /// The PROV-DM type of a record.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        #[non_exhaustive]
        pub enum Kind {
            $(#[doc = concat!("`", $keyword, "`.")] $name,)*
        }

        impl Kind {
            /// Every kind: the elements first, then the relations in PROV-DM
            /// order.
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

            /// Whether PROV-DM gives the kind no identifier: `alternateOf`,
            /// `specializationOf`, `mentionOf` and `hadMember`. The PROV-N
            /// grammar has no identifier for them, so `Profile::Strict`
            /// rejects one, and `unified()` never merges them.
            pub(crate) fn is_bare(self) -> bool {
                match self {
                    $(Kind::$name => records!(@bare $($bare)?),)*
                }
            }

            /// Argument counts PROV-N allows after the identifier.
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
                $(#[doc = formal_doc!($attr)] pub $field: Option<formal_type!($attr)>,)*
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
        start_time: StartTime,
        end_time: EndTime,
    }

    /// An agent.
    Agent "agent" element [0] {}

    /// A generation (`wasGeneratedBy`).
    Generation "wasGeneratedBy" relation [1, 3] {
        entity: Entity,
        activity: Activity,
        time: Time,
    }

    /// A usage (`used`).
    Usage "used" relation [1, 3] {
        activity: Activity,
        entity: Entity,
        time: Time,
    }

    /// A communication (`wasInformedBy`).
    Communication "wasInformedBy" relation [2] {
        informed: Informed,
        informant: Informant,
    }

    /// A start (`wasStartedBy`).
    Start "wasStartedBy" relation [1, 4] {
        activity: Activity,
        trigger: Trigger,
        starter: Starter,
        time: Time,
    }

    /// An end (`wasEndedBy`).
    End "wasEndedBy" relation [1, 4] {
        activity: Activity,
        trigger: Trigger,
        ender: Ender,
        time: Time,
    }

    /// An invalidation (`wasInvalidatedBy`).
    Invalidation "wasInvalidatedBy" relation [1, 3] {
        entity: Entity,
        activity: Activity,
        time: Time,
    }

    /// A derivation (`wasDerivedFrom`).
    Derivation "wasDerivedFrom" relation [2, 5] {
        generated_entity: GeneratedEntity,
        used_entity: UsedEntity,
        activity: Activity,
        generation: Generation,
        usage: Usage,
    }

    /// An attribution (`wasAttributedTo`).
    Attribution "wasAttributedTo" relation [2] {
        entity: Entity,
        agent: Agent,
    }

    /// An association (`wasAssociatedWith`).
    Association "wasAssociatedWith" relation [1, 3] {
        activity: Activity,
        agent: Agent,
        plan: Plan,
    }

    /// A delegation (`actedOnBehalfOf`).
    Delegation "actedOnBehalfOf" relation [2, 3] {
        delegate: Delegate,
        responsible: Responsible,
        activity: Activity,
    }

    /// An influence (`wasInfluencedBy`).
    Influence "wasInfluencedBy" relation [2] {
        influencee: Influencee,
        influencer: Influencer,
    }

    /// A specialisation (`specializationOf`).
    Specialization "specializationOf" relation bare [2] {
        specific_entity: SpecificEntity,
        general_entity: GeneralEntity,
    }

    /// An alternate (`alternateOf`).
    Alternate "alternateOf" relation bare [2] {
        alternate1: Alternate1,
        alternate2: Alternate2,
    }

    /// A mention (`mentionOf`).
    Mention "mentionOf" relation bare [3] {
        specific_entity: SpecificEntity,
        general_entity: GeneralEntity,
        bundle: Bundle,
    }

    /// A membership (`hadMember`).
    Membership "hadMember" relation bare [2] {
        collection: Collection,
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
                    "{attr} is given as an attribute; its value belongs in the positional argument"
                ),
            )),
            None => Ok(()),
        }
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

#[cfg(test)]
mod tests;
