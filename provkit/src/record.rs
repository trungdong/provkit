//! Records: the eighteen PROV-DM types and the enum that holds them.

use crate::namespaces::PROV;
use crate::{Attributes, DateTime, Error, QName, Result};

/// The PROV-DM type of a record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    /// `entity`.
    Entity,
    /// `activity`.
    Activity,
    /// `agent`.
    Agent,
    /// `wasGeneratedBy`.
    Generation,
    /// `used`.
    Usage,
    /// `wasInformedBy`.
    Communication,
    /// `wasStartedBy`.
    Start,
    /// `wasEndedBy`.
    End,
    /// `wasInvalidatedBy`.
    Invalidation,
    /// `wasDerivedFrom`.
    Derivation,
    /// `wasAttributedTo`.
    Attribution,
    /// `wasAssociatedWith`.
    Association,
    /// `actedOnBehalfOf`.
    Delegation,
    /// `wasInfluencedBy`.
    Influence,
    /// `specializationOf`.
    Specialization,
    /// `alternateOf`.
    Alternate,
    /// `mentionOf`.
    Mention,
    /// `hadMember`.
    Membership,
}

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

impl Kind {
    /// Every kind, elements first, then relations in PROV-DM order.
    pub const ALL: [Kind; 18] = [
        Kind::Entity,
        Kind::Activity,
        Kind::Agent,
        Kind::Generation,
        Kind::Usage,
        Kind::Communication,
        Kind::Start,
        Kind::End,
        Kind::Invalidation,
        Kind::Derivation,
        Kind::Attribution,
        Kind::Association,
        Kind::Delegation,
        Kind::Influence,
        Kind::Specialization,
        Kind::Alternate,
        Kind::Mention,
        Kind::Membership,
    ];

    /// The PROV-JSON section key, which is also the PROV-N keyword.
    pub fn json_key(self) -> &'static str {
        match self {
            Kind::Entity => "entity",
            Kind::Activity => "activity",
            Kind::Agent => "agent",
            Kind::Generation => "wasGeneratedBy",
            Kind::Usage => "used",
            Kind::Communication => "wasInformedBy",
            Kind::Start => "wasStartedBy",
            Kind::End => "wasEndedBy",
            Kind::Invalidation => "wasInvalidatedBy",
            Kind::Derivation => "wasDerivedFrom",
            Kind::Attribution => "wasAttributedTo",
            Kind::Association => "wasAssociatedWith",
            Kind::Delegation => "actedOnBehalfOf",
            Kind::Influence => "wasInfluencedBy",
            Kind::Specialization => "specializationOf",
            Kind::Alternate => "alternateOf",
            Kind::Mention => "mentionOf",
            Kind::Membership => "hadMember",
        }
    }

    /// The PROV-N statement keyword (the same text as the JSON key).
    pub fn provn_keyword(self) -> &'static str {
        self.json_key()
    }

    /// The kind for a JSON key or PROV-N keyword.
    pub fn from_json_key(key: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|k| k.json_key() == key)
    }

    /// Entity, activity and agent; everything else is a relation.
    pub fn is_element(self) -> bool {
        matches!(self, Kind::Entity | Kind::Activity | Kind::Agent)
    }

    /// The kind's formal attributes, in PROV-DM order.
    // One row per relation mirrors the PROV-DM table, even where rows coincide.
    #[allow(clippy::match_same_arms)]
    pub fn formals(self) -> &'static [FormalAttr] {
        match self {
            Kind::Entity | Kind::Agent => &[],
            Kind::Activity => &[F::StartTime, F::EndTime],
            Kind::Generation => &[F::Entity, F::Activity, F::Time],
            Kind::Usage => &[F::Activity, F::Entity, F::Time],
            Kind::Communication => &[F::Informed, F::Informant],
            Kind::Start => &[F::Activity, F::Trigger, F::Starter, F::Time],
            Kind::End => &[F::Activity, F::Trigger, F::Ender, F::Time],
            Kind::Invalidation => &[F::Entity, F::Activity, F::Time],
            Kind::Derivation => &[
                F::GeneratedEntity,
                F::UsedEntity,
                F::Activity,
                F::Generation,
                F::Usage,
            ],
            Kind::Attribution => &[F::Entity, F::Agent],
            Kind::Association => &[F::Activity, F::Agent, F::Plan],
            Kind::Delegation => &[F::Delegate, F::Responsible, F::Activity],
            Kind::Influence => &[F::Influencee, F::Influencer],
            Kind::Specialization => &[F::SpecificEntity, F::GeneralEntity],
            Kind::Alternate => &[F::Alternate1, F::Alternate2],
            Kind::Mention => &[F::SpecificEntity, F::GeneralEntity, F::Bundle],
            Kind::Membership => &[F::Collection, F::Entity],
        }
    }
}

impl FormalAttr {
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
        ALL_FORMALS.into_iter().find(|f| f.local_name() == name)
    }

    /// Whether the value is a dateTime rather than a name.
    pub fn is_time(self) -> bool {
        matches!(self, F::Time | F::StartTime | F::EndTime)
    }
}

/// The formal of `kind` that `name` spells, when `name` is `prov:` plus the
/// local name of one of the kind's formals.
pub(crate) fn formal_named(kind: Kind, name: &QName) -> Option<FormalAttr> {
    if &**name.namespace() != PROV {
        return None;
    }
    kind.formals()
        .iter()
        .copied()
        .find(|f| f.local_name() == name.local())
}

/// The error for an attribute that spells one of the record's own formals.
pub(crate) fn formal_attribute_error(kind: Kind, attr: FormalAttr) -> Error {
    structure(format!(
        "{} has an attribute prov:{}; its value belongs in the positional argument",
        kind.json_key(),
        attr.local_name()
    ))
}

const ALL_FORMALS: [FormalAttr; 26] = [
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

/// An entity.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Entity {
    /// The identifier.
    pub id: QName,
    /// Non-formal attributes.
    pub attributes: Attributes,
}

/// An activity.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Activity {
    /// The identifier.
    pub id: QName,
    /// `prov:startTime`.
    pub start_time: Option<DateTime>,
    /// `prov:endTime`.
    pub end_time: Option<DateTime>,
    /// Non-formal attributes.
    pub attributes: Attributes,
}

/// An agent.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Agent {
    /// The identifier.
    pub id: QName,
    /// Non-formal attributes.
    pub attributes: Attributes,
}

/// A generation (`wasGeneratedBy`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Generation {
    /// The identifier.
    pub id: Option<QName>,
    /// `prov:entity`.
    pub entity: Option<QName>,
    /// `prov:activity`.
    pub activity: Option<QName>,
    /// `prov:time`.
    pub time: Option<DateTime>,
    /// Non-formal attributes.
    pub attributes: Attributes,
}

/// A usage (`used`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Usage {
    /// The identifier.
    pub id: Option<QName>,
    /// `prov:activity`.
    pub activity: Option<QName>,
    /// `prov:entity`.
    pub entity: Option<QName>,
    /// `prov:time`.
    pub time: Option<DateTime>,
    /// Non-formal attributes.
    pub attributes: Attributes,
}

/// A communication (`wasInformedBy`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Communication {
    /// The identifier.
    pub id: Option<QName>,
    /// `prov:informed`.
    pub informed: Option<QName>,
    /// `prov:informant`.
    pub informant: Option<QName>,
    /// Non-formal attributes.
    pub attributes: Attributes,
}

/// A start (`wasStartedBy`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Start {
    /// The identifier.
    pub id: Option<QName>,
    /// `prov:activity`.
    pub activity: Option<QName>,
    /// `prov:trigger`.
    pub trigger: Option<QName>,
    /// `prov:starter`.
    pub starter: Option<QName>,
    /// `prov:time`.
    pub time: Option<DateTime>,
    /// Non-formal attributes.
    pub attributes: Attributes,
}

/// An end (`wasEndedBy`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct End {
    /// The identifier.
    pub id: Option<QName>,
    /// `prov:activity`.
    pub activity: Option<QName>,
    /// `prov:trigger`.
    pub trigger: Option<QName>,
    /// `prov:ender`.
    pub ender: Option<QName>,
    /// `prov:time`.
    pub time: Option<DateTime>,
    /// Non-formal attributes.
    pub attributes: Attributes,
}

/// An invalidation (`wasInvalidatedBy`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Invalidation {
    /// The identifier.
    pub id: Option<QName>,
    /// `prov:entity`.
    pub entity: Option<QName>,
    /// `prov:activity`.
    pub activity: Option<QName>,
    /// `prov:time`.
    pub time: Option<DateTime>,
    /// Non-formal attributes.
    pub attributes: Attributes,
}

/// A derivation (`wasDerivedFrom`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Derivation {
    /// The identifier.
    pub id: Option<QName>,
    /// `prov:generatedEntity`.
    pub generated_entity: Option<QName>,
    /// `prov:usedEntity`.
    pub used_entity: Option<QName>,
    /// `prov:activity`.
    pub activity: Option<QName>,
    /// `prov:generation`.
    pub generation: Option<QName>,
    /// `prov:usage`.
    pub usage: Option<QName>,
    /// Non-formal attributes.
    pub attributes: Attributes,
}

/// An attribution (`wasAttributedTo`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Attribution {
    /// The identifier.
    pub id: Option<QName>,
    /// `prov:entity`.
    pub entity: Option<QName>,
    /// `prov:agent`.
    pub agent: Option<QName>,
    /// Non-formal attributes.
    pub attributes: Attributes,
}

/// An association (`wasAssociatedWith`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Association {
    /// The identifier.
    pub id: Option<QName>,
    /// `prov:activity`.
    pub activity: Option<QName>,
    /// `prov:agent`.
    pub agent: Option<QName>,
    /// `prov:plan`.
    pub plan: Option<QName>,
    /// Non-formal attributes.
    pub attributes: Attributes,
}

/// A delegation (`actedOnBehalfOf`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Delegation {
    /// The identifier.
    pub id: Option<QName>,
    /// `prov:delegate`.
    pub delegate: Option<QName>,
    /// `prov:responsible`.
    pub responsible: Option<QName>,
    /// `prov:activity`.
    pub activity: Option<QName>,
    /// Non-formal attributes.
    pub attributes: Attributes,
}

/// An influence (`wasInfluencedBy`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Influence {
    /// The identifier.
    pub id: Option<QName>,
    /// `prov:influencee`.
    pub influencee: Option<QName>,
    /// `prov:influencer`.
    pub influencer: Option<QName>,
    /// Non-formal attributes.
    pub attributes: Attributes,
}

/// A specialisation (`specializationOf`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Specialization {
    /// The identifier.
    pub id: Option<QName>,
    /// `prov:specificEntity`.
    pub specific_entity: Option<QName>,
    /// `prov:generalEntity`.
    pub general_entity: Option<QName>,
    /// Non-formal attributes.
    pub attributes: Attributes,
}

/// An alternate (`alternateOf`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Alternate {
    /// The identifier.
    pub id: Option<QName>,
    /// `prov:alternate1`.
    pub alternate1: Option<QName>,
    /// `prov:alternate2`.
    pub alternate2: Option<QName>,
    /// Non-formal attributes.
    pub attributes: Attributes,
}

/// A mention (`mentionOf`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Mention {
    /// The identifier.
    pub id: Option<QName>,
    /// `prov:specificEntity`.
    pub specific_entity: Option<QName>,
    /// `prov:generalEntity`.
    pub general_entity: Option<QName>,
    /// `prov:bundle`.
    pub bundle: Option<QName>,
    /// Non-formal attributes.
    pub attributes: Attributes,
}

/// A membership (`hadMember`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Membership {
    /// The identifier.
    pub id: Option<QName>,
    /// `prov:collection`.
    pub collection: Option<QName>,
    /// `prov:entity`.
    pub entity: Option<QName>,
    /// Non-formal attributes.
    pub attributes: Attributes,
}

/// A PROV record of any of the eighteen PROV-DM types.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Record {
    /// An entity.
    Entity(Entity),
    /// An activity.
    Activity(Activity),
    /// An agent.
    Agent(Agent),
    /// A generation.
    Generation(Generation),
    /// A usage.
    Usage(Usage),
    /// A communication.
    Communication(Communication),
    /// A start.
    Start(Start),
    /// An end.
    End(End),
    /// An invalidation.
    Invalidation(Invalidation),
    /// A derivation.
    Derivation(Derivation),
    /// An attribution.
    Attribution(Attribution),
    /// An association.
    Association(Association),
    /// A delegation.
    Delegation(Delegation),
    /// An influence.
    Influence(Influence),
    /// A specialisation.
    Specialization(Specialization),
    /// An alternate.
    Alternate(Alternate),
    /// A mention.
    Mention(Mention),
    /// A membership.
    Membership(Membership),
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
                return Err(structure(format!(
                    "{} has no formal attribute prov:{}",
                    kind.json_key(),
                    attr.local_name()
                )));
            }
            let time_expected = attr.is_time();
            if time_expected != matches!(value, Formal::Time(_)) {
                return Err(structure(format!(
                    "prov:{} needs a {}",
                    attr.local_name(),
                    if time_expected { "dateTime" } else { "name" }
                )));
            }
            if items[..i].iter().any(|(a, _)| a == attr) {
                return Err(structure(format!("prov:{} given twice", attr.local_name())));
            }
        }
        Ok(Formals { items })
    }

    fn name(&mut self, attr: FormalAttr) -> Option<QName> {
        let i = self.items.iter().position(|(a, _)| *a == attr)?;
        match self.items.remove(i).1 {
            Formal::Name(q) => Some(q),
            Formal::Time(_) => unreachable!("checked in Formals::new"),
        }
    }

    fn time(&mut self, attr: FormalAttr) -> Option<DateTime> {
        let i = self.items.iter().position(|(a, _)| *a == attr)?;
        match self.items.remove(i).1 {
            Formal::Time(t) => Some(t),
            Formal::Name(_) => unreachable!("checked in Formals::new"),
        }
    }
}

fn structure(message: String) -> Error {
    Error::Structure {
        format: "record",
        message,
    }
}

/// Collects present formals for [`Record::formals`].
struct FormalList<'a>(Vec<(FormalAttr, FormalRef<'a>)>);

impl<'a> FormalList<'a> {
    fn name(&mut self, attr: FormalAttr, value: Option<&'a QName>) {
        if let Some(q) = value {
            self.0.push((attr, FormalRef::Name(q)));
        }
    }

    fn time(&mut self, attr: FormalAttr, value: Option<&'a DateTime>) {
        if let Some(t) = value {
            self.0.push((attr, FormalRef::Time(t)));
        }
    }
}

impl Record {
    /// Build a record of `kind` from its parts. Elements need an identifier;
    /// formals must belong to the kind, have the right value type, and appear
    /// at most once. No attribute may be named `prov:` plus the local name of
    /// one of the kind's formals (`prov:activity` on `wasGeneratedBy`).
    // One arm per kind; splitting it would scatter the table.
    #[allow(clippy::too_many_lines)]
    pub fn from_parts(
        kind: Kind,
        id: Option<QName>,
        formals: Vec<(FormalAttr, Formal)>,
        attributes: Attributes,
    ) -> Result<Record> {
        let mut f = Formals::new(kind, formals)?;
        if let Some(attr) = attributes.iter().find_map(|(n, _)| formal_named(kind, n)) {
            return Err(formal_attribute_error(kind, attr));
        }
        let element_id = |id: Option<QName>| {
            id.ok_or_else(|| structure(format!("{} needs an identifier", kind.json_key())))
        };
        Ok(match kind {
            Kind::Entity => Record::Entity(Entity {
                id: element_id(id)?,
                attributes,
            }),
            Kind::Activity => Record::Activity(Activity {
                id: element_id(id)?,
                start_time: f.time(F::StartTime),
                end_time: f.time(F::EndTime),
                attributes,
            }),
            Kind::Agent => Record::Agent(Agent {
                id: element_id(id)?,
                attributes,
            }),
            Kind::Generation => Record::Generation(Generation {
                id,
                entity: f.name(F::Entity),
                activity: f.name(F::Activity),
                time: f.time(F::Time),
                attributes,
            }),
            Kind::Usage => Record::Usage(Usage {
                id,
                activity: f.name(F::Activity),
                entity: f.name(F::Entity),
                time: f.time(F::Time),
                attributes,
            }),
            Kind::Communication => Record::Communication(Communication {
                id,
                informed: f.name(F::Informed),
                informant: f.name(F::Informant),
                attributes,
            }),
            Kind::Start => Record::Start(Start {
                id,
                activity: f.name(F::Activity),
                trigger: f.name(F::Trigger),
                starter: f.name(F::Starter),
                time: f.time(F::Time),
                attributes,
            }),
            Kind::End => Record::End(End {
                id,
                activity: f.name(F::Activity),
                trigger: f.name(F::Trigger),
                ender: f.name(F::Ender),
                time: f.time(F::Time),
                attributes,
            }),
            Kind::Invalidation => Record::Invalidation(Invalidation {
                id,
                entity: f.name(F::Entity),
                activity: f.name(F::Activity),
                time: f.time(F::Time),
                attributes,
            }),
            Kind::Derivation => Record::Derivation(Derivation {
                id,
                generated_entity: f.name(F::GeneratedEntity),
                used_entity: f.name(F::UsedEntity),
                activity: f.name(F::Activity),
                generation: f.name(F::Generation),
                usage: f.name(F::Usage),
                attributes,
            }),
            Kind::Attribution => Record::Attribution(Attribution {
                id,
                entity: f.name(F::Entity),
                agent: f.name(F::Agent),
                attributes,
            }),
            Kind::Association => Record::Association(Association {
                id,
                activity: f.name(F::Activity),
                agent: f.name(F::Agent),
                plan: f.name(F::Plan),
                attributes,
            }),
            Kind::Delegation => Record::Delegation(Delegation {
                id,
                delegate: f.name(F::Delegate),
                responsible: f.name(F::Responsible),
                activity: f.name(F::Activity),
                attributes,
            }),
            Kind::Influence => Record::Influence(Influence {
                id,
                influencee: f.name(F::Influencee),
                influencer: f.name(F::Influencer),
                attributes,
            }),
            Kind::Specialization => Record::Specialization(Specialization {
                id,
                specific_entity: f.name(F::SpecificEntity),
                general_entity: f.name(F::GeneralEntity),
                attributes,
            }),
            Kind::Alternate => Record::Alternate(Alternate {
                id,
                alternate1: f.name(F::Alternate1),
                alternate2: f.name(F::Alternate2),
                attributes,
            }),
            Kind::Mention => Record::Mention(Mention {
                id,
                specific_entity: f.name(F::SpecificEntity),
                general_entity: f.name(F::GeneralEntity),
                bundle: f.name(F::Bundle),
                attributes,
            }),
            Kind::Membership => Record::Membership(Membership {
                id,
                collection: f.name(F::Collection),
                entity: f.name(F::Entity),
                attributes,
            }),
        })
    }

    /// The record's kind.
    pub fn kind(&self) -> Kind {
        match self {
            Record::Entity(_) => Kind::Entity,
            Record::Activity(_) => Kind::Activity,
            Record::Agent(_) => Kind::Agent,
            Record::Generation(_) => Kind::Generation,
            Record::Usage(_) => Kind::Usage,
            Record::Communication(_) => Kind::Communication,
            Record::Start(_) => Kind::Start,
            Record::End(_) => Kind::End,
            Record::Invalidation(_) => Kind::Invalidation,
            Record::Derivation(_) => Kind::Derivation,
            Record::Attribution(_) => Kind::Attribution,
            Record::Association(_) => Kind::Association,
            Record::Delegation(_) => Kind::Delegation,
            Record::Influence(_) => Kind::Influence,
            Record::Specialization(_) => Kind::Specialization,
            Record::Alternate(_) => Kind::Alternate,
            Record::Mention(_) => Kind::Mention,
            Record::Membership(_) => Kind::Membership,
        }
    }

    /// The identifier, if any.
    pub fn id(&self) -> Option<&QName> {
        match self {
            Record::Entity(r) => Some(&r.id),
            Record::Activity(r) => Some(&r.id),
            Record::Agent(r) => Some(&r.id),
            Record::Generation(r) => r.id.as_ref(),
            Record::Usage(r) => r.id.as_ref(),
            Record::Communication(r) => r.id.as_ref(),
            Record::Start(r) => r.id.as_ref(),
            Record::End(r) => r.id.as_ref(),
            Record::Invalidation(r) => r.id.as_ref(),
            Record::Derivation(r) => r.id.as_ref(),
            Record::Attribution(r) => r.id.as_ref(),
            Record::Association(r) => r.id.as_ref(),
            Record::Delegation(r) => r.id.as_ref(),
            Record::Influence(r) => r.id.as_ref(),
            Record::Specialization(r) => r.id.as_ref(),
            Record::Alternate(r) => r.id.as_ref(),
            Record::Mention(r) => r.id.as_ref(),
            Record::Membership(r) => r.id.as_ref(),
        }
    }

    /// Non-formal attributes.
    pub fn attributes(&self) -> &Attributes {
        match self {
            Record::Entity(r) => &r.attributes,
            Record::Activity(r) => &r.attributes,
            Record::Agent(r) => &r.attributes,
            Record::Generation(r) => &r.attributes,
            Record::Usage(r) => &r.attributes,
            Record::Communication(r) => &r.attributes,
            Record::Start(r) => &r.attributes,
            Record::End(r) => &r.attributes,
            Record::Invalidation(r) => &r.attributes,
            Record::Derivation(r) => &r.attributes,
            Record::Attribution(r) => &r.attributes,
            Record::Association(r) => &r.attributes,
            Record::Delegation(r) => &r.attributes,
            Record::Influence(r) => &r.attributes,
            Record::Specialization(r) => &r.attributes,
            Record::Alternate(r) => &r.attributes,
            Record::Mention(r) => &r.attributes,
            Record::Membership(r) => &r.attributes,
        }
    }

    /// Non-formal attributes, mutably.
    pub fn attributes_mut(&mut self) -> &mut Attributes {
        match self {
            Record::Entity(r) => &mut r.attributes,
            Record::Activity(r) => &mut r.attributes,
            Record::Agent(r) => &mut r.attributes,
            Record::Generation(r) => &mut r.attributes,
            Record::Usage(r) => &mut r.attributes,
            Record::Communication(r) => &mut r.attributes,
            Record::Start(r) => &mut r.attributes,
            Record::End(r) => &mut r.attributes,
            Record::Invalidation(r) => &mut r.attributes,
            Record::Derivation(r) => &mut r.attributes,
            Record::Attribution(r) => &mut r.attributes,
            Record::Association(r) => &mut r.attributes,
            Record::Delegation(r) => &mut r.attributes,
            Record::Influence(r) => &mut r.attributes,
            Record::Specialization(r) => &mut r.attributes,
            Record::Alternate(r) => &mut r.attributes,
            Record::Mention(r) => &mut r.attributes,
            Record::Membership(r) => &mut r.attributes,
        }
    }

    /// The formal attributes that are present, in table order.
    pub fn formals(&self) -> Vec<(FormalAttr, FormalRef<'_>)> {
        let mut out = FormalList(Vec::new());
        match self {
            Record::Entity(_) | Record::Agent(_) => {}
            Record::Activity(r) => {
                out.time(F::StartTime, r.start_time.as_ref());
                out.time(F::EndTime, r.end_time.as_ref());
            }
            Record::Generation(r) => {
                out.name(F::Entity, r.entity.as_ref());
                out.name(F::Activity, r.activity.as_ref());
                out.time(F::Time, r.time.as_ref());
            }
            Record::Usage(r) => {
                out.name(F::Activity, r.activity.as_ref());
                out.name(F::Entity, r.entity.as_ref());
                out.time(F::Time, r.time.as_ref());
            }
            Record::Communication(r) => {
                out.name(F::Informed, r.informed.as_ref());
                out.name(F::Informant, r.informant.as_ref());
            }
            Record::Start(r) => {
                out.name(F::Activity, r.activity.as_ref());
                out.name(F::Trigger, r.trigger.as_ref());
                out.name(F::Starter, r.starter.as_ref());
                out.time(F::Time, r.time.as_ref());
            }
            Record::End(r) => {
                out.name(F::Activity, r.activity.as_ref());
                out.name(F::Trigger, r.trigger.as_ref());
                out.name(F::Ender, r.ender.as_ref());
                out.time(F::Time, r.time.as_ref());
            }
            Record::Invalidation(r) => {
                out.name(F::Entity, r.entity.as_ref());
                out.name(F::Activity, r.activity.as_ref());
                out.time(F::Time, r.time.as_ref());
            }
            Record::Derivation(r) => {
                out.name(F::GeneratedEntity, r.generated_entity.as_ref());
                out.name(F::UsedEntity, r.used_entity.as_ref());
                out.name(F::Activity, r.activity.as_ref());
                out.name(F::Generation, r.generation.as_ref());
                out.name(F::Usage, r.usage.as_ref());
            }
            Record::Attribution(r) => {
                out.name(F::Entity, r.entity.as_ref());
                out.name(F::Agent, r.agent.as_ref());
            }
            Record::Association(r) => {
                out.name(F::Activity, r.activity.as_ref());
                out.name(F::Agent, r.agent.as_ref());
                out.name(F::Plan, r.plan.as_ref());
            }
            Record::Delegation(r) => {
                out.name(F::Delegate, r.delegate.as_ref());
                out.name(F::Responsible, r.responsible.as_ref());
                out.name(F::Activity, r.activity.as_ref());
            }
            Record::Influence(r) => {
                out.name(F::Influencee, r.influencee.as_ref());
                out.name(F::Influencer, r.influencer.as_ref());
            }
            Record::Specialization(r) => {
                out.name(F::SpecificEntity, r.specific_entity.as_ref());
                out.name(F::GeneralEntity, r.general_entity.as_ref());
            }
            Record::Alternate(r) => {
                out.name(F::Alternate1, r.alternate1.as_ref());
                out.name(F::Alternate2, r.alternate2.as_ref());
            }
            Record::Mention(r) => {
                out.name(F::SpecificEntity, r.specific_entity.as_ref());
                out.name(F::GeneralEntity, r.general_entity.as_ref());
                out.name(F::Bundle, r.bundle.as_ref());
            }
            Record::Membership(r) => {
                out.name(F::Collection, r.collection.as_ref());
                out.name(F::Entity, r.entity.as_ref());
            }
        }
        out.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::{hash, q};
    use crate::{Attributes, Value};

    fn t() -> DateTime {
        DateTime::parse("2011-11-16T16:05:33Z").unwrap()
    }

    /// A value for `attr` whose text names the formal, so a swapped field shows.
    fn formal_for(attr: FormalAttr) -> Formal {
        if attr.is_time() {
            let minute =
                u8::try_from(ALL_FORMALS.iter().position(|f| *f == attr).unwrap()).unwrap();
            Formal::Time(DateTime::new(2011, 11, 16, 16, minute, 0, 0, Some(0)).unwrap())
        } else {
            Formal::Name(q(attr.local_name()))
        }
    }

    fn owned(r: FormalRef<'_>) -> Formal {
        match r {
            FormalRef::Name(q) => Formal::Name(q.clone()),
            FormalRef::Time(t) => Formal::Time(t.clone()),
        }
    }

    #[test]
    fn tables() {
        assert_eq!(Kind::ALL.len(), 18);
        assert_eq!(
            Kind::from_json_key("wasGeneratedBy"),
            Some(Kind::Generation)
        );
        assert_eq!(Kind::from_json_key("wasGenerated"), None);
        assert_eq!(Kind::Entity.json_key(), "entity");
        assert_eq!(Kind::Generation.json_key(), "wasGeneratedBy");
        assert_eq!(Kind::Generation.provn_keyword(), "wasGeneratedBy");
        assert_eq!(Kind::Mention.provn_keyword(), "mentionOf");
        assert_eq!(
            Kind::Generation.formals(),
            &[FormalAttr::Entity, FormalAttr::Activity, FormalAttr::Time]
        );
        assert_eq!(Kind::Derivation.formals().len(), 5);
        assert!(Kind::Entity.is_element() && !Kind::Usage.is_element());
        assert_eq!(
            FormalAttr::from_local_name("generatedEntity"),
            Some(FormalAttr::GeneratedEntity)
        );
        assert!(FormalAttr::Time.is_time() && !FormalAttr::Entity.is_time());
    }

    #[test]
    fn keys_and_local_names_round_trip() {
        for kind in Kind::ALL {
            assert_eq!(Kind::from_json_key(kind.json_key()), Some(kind));
        }
        for attr in ALL_FORMALS {
            assert_eq!(FormalAttr::from_local_name(attr.local_name()), Some(attr));
        }
        let times: Vec<_> = ALL_FORMALS.into_iter().filter(|f| f.is_time()).collect();
        assert_eq!(times, [F::Time, F::StartTime, F::EndTime]);
    }

    #[test]
    fn formal_tables_match_the_spec() {
        let table: Vec<(Kind, &[&str])> = vec![
            (Kind::Entity, &[]),
            (Kind::Activity, &["startTime", "endTime"]),
            (Kind::Agent, &[]),
            (Kind::Generation, &["entity", "activity", "time"]),
            (Kind::Usage, &["activity", "entity", "time"]),
            (Kind::Communication, &["informed", "informant"]),
            (Kind::Start, &["activity", "trigger", "starter", "time"]),
            (Kind::End, &["activity", "trigger", "ender", "time"]),
            (Kind::Invalidation, &["entity", "activity", "time"]),
            (
                Kind::Derivation,
                &[
                    "generatedEntity",
                    "usedEntity",
                    "activity",
                    "generation",
                    "usage",
                ],
            ),
            (Kind::Attribution, &["entity", "agent"]),
            (Kind::Association, &["activity", "agent", "plan"]),
            (Kind::Delegation, &["delegate", "responsible", "activity"]),
            (Kind::Influence, &["influencee", "influencer"]),
            (Kind::Specialization, &["specificEntity", "generalEntity"]),
            (Kind::Alternate, &["alternate1", "alternate2"]),
            (
                Kind::Mention,
                &["specificEntity", "generalEntity", "bundle"],
            ),
            (Kind::Membership, &["collection", "entity"]),
        ];
        assert_eq!(table.iter().map(|(k, _)| *k).collect::<Vec<_>>(), Kind::ALL);
        for (kind, names) in table {
            let got: Vec<_> = kind.formals().iter().map(|f| f.local_name()).collect();
            assert_eq!(got, names, "{kind:?}");
        }
    }

    #[test]
    fn from_parts_builds_a_generation() {
        let r = Record::from_parts(
            Kind::Generation,
            Some(q("g1")),
            vec![
                (FormalAttr::Entity, Formal::Name(q("e1"))),
                (FormalAttr::Time, Formal::Time(t())),
            ],
            Attributes::new(),
        )
        .unwrap();
        assert_eq!(r.kind(), Kind::Generation);
        assert_eq!(r.id(), Some(&q("g1")));
        let Record::Generation(g) = &r else { panic!() };
        assert_eq!(g.entity, Some(q("e1")));
        assert_eq!(g.activity, None);
        assert_eq!(g.time, Some(t()));
        assert_eq!(
            r.formals(),
            vec![
                (FormalAttr::Entity, FormalRef::Name(&q("e1"))),
                (FormalAttr::Time, FormalRef::Time(&t())),
            ]
        );
    }

    #[test]
    fn from_parts_builds_every_kind_empty() {
        for kind in Kind::ALL {
            let id = kind.is_element().then(|| q("x"));
            let r = Record::from_parts(kind, id.clone(), vec![], Attributes::new()).unwrap();
            assert_eq!(r.kind(), kind);
            assert_eq!(r.id(), id.as_ref());
            assert!(r.formals().is_empty());
            assert!(r.attributes().is_empty());
        }
    }

    #[test]
    fn formals_round_trip_through_from_parts_for_every_kind() {
        for kind in Kind::ALL {
            // Given in reverse, so table order in the output is not an echo of the input.
            let parts: Vec<_> = kind
                .formals()
                .iter()
                .rev()
                .map(|&a| (a, formal_for(a)))
                .collect();
            let mut attributes = Attributes::new();
            attributes.insert(q("label"), Value::from("x"));
            let r = Record::from_parts(kind, Some(q("r")), parts, attributes.clone()).unwrap();
            assert_eq!(r.kind(), kind);
            assert_eq!(r.id(), Some(&q("r")));
            assert_eq!(r.attributes(), &attributes);

            let formals = r.formals();
            let attrs: Vec<_> = formals.iter().map(|(a, _)| *a).collect();
            assert_eq!(attrs, kind.formals(), "{kind:?} order");
            for (a, v) in &formals {
                assert_eq!(owned(*v), formal_for(*a), "{kind:?} {a:?}");
            }

            let rebuilt = Record::from_parts(
                kind,
                r.id().cloned(),
                formals.into_iter().map(|(a, v)| (a, owned(v))).collect(),
                r.attributes().clone(),
            )
            .unwrap();
            assert_eq!(rebuilt, r, "{kind:?}");
            assert_eq!(hash(&rebuilt), hash(&r), "{kind:?}");
        }
    }

    #[test]
    fn from_parts_rejects_bad_parts() {
        let bad = |kind, id, formals| {
            matches!(
                Record::from_parts(kind, id, formals, Attributes::new()),
                Err(Error::Structure { .. })
            )
        };
        assert!(bad(Kind::Entity, None, vec![]));
        assert!(bad(Kind::Activity, None, vec![]));
        assert!(bad(Kind::Agent, None, vec![]));
        assert!(bad(
            Kind::Generation,
            None,
            vec![(FormalAttr::Plan, Formal::Name(q("p")))]
        ));
        assert!(bad(
            Kind::Entity,
            Some(q("e")),
            vec![(FormalAttr::Entity, Formal::Name(q("e")))]
        ));
        assert!(bad(
            Kind::Generation,
            None,
            vec![(FormalAttr::Entity, Formal::Time(t()))]
        ));
        assert!(bad(
            Kind::Generation,
            None,
            vec![(FormalAttr::Time, Formal::Name(q("t")))]
        ));
        assert!(bad(
            Kind::Generation,
            None,
            vec![
                (FormalAttr::Entity, Formal::Name(q("a"))),
                (FormalAttr::Entity, Formal::Name(q("b"))),
            ]
        ));
    }

    #[test]
    fn attribute_named_after_an_own_formal_is_rejected() {
        let prov = |l: &str| QName::new(PROV, l);
        let mut clash = Attributes::new();
        clash.insert(prov("activity"), Value::QName(q("a2")));
        match Record::from_parts(Kind::Generation, None, vec![], clash) {
            Err(Error::Structure { format, message }) => {
                assert_eq!(format, "record");
                assert!(message.contains("prov:activity"), "{message}");
                assert!(message.contains("positional argument"), "{message}");
            }
            other => panic!("expected a structure error, got {other:?}"),
        }
        // A formal of another kind, and prov:type and prov:label, are plain attributes.
        let mut fine = Attributes::new();
        fine.insert(prov("plan"), Value::QName(q("p")));
        fine.insert(prov("type"), Value::from("t"));
        fine.insert(prov("label"), Value::from("l"));
        for kind in [Kind::Generation, Kind::Usage, Kind::Entity] {
            let r = Record::from_parts(kind, Some(q("x")), vec![], fine.clone()).unwrap();
            assert_eq!(r.attributes().len(), 3);
        }
    }

    #[test]
    fn records_compare_and_hash_by_value() {
        let build = |id: &str| {
            Record::from_parts(
                Kind::Usage,
                Some(q(id)),
                vec![(FormalAttr::Activity, Formal::Name(q("a")))],
                Attributes::new(),
            )
            .unwrap()
        };
        let r = build("u");
        assert_eq!(r.clone(), r);
        assert_eq!(hash(&r.clone()), hash(&r));
        assert_ne!(build("u"), build("v"));
        let unnamed = Record::from_parts(
            Kind::Usage,
            None,
            r.formals()
                .into_iter()
                .map(|(a, v)| (a, owned(v)))
                .collect(),
            Attributes::new(),
        )
        .unwrap();
        assert_ne!(unnamed, r);
    }

    #[test]
    fn attributes_mut_edits_every_variant() {
        for kind in Kind::ALL {
            let mut r = Record::from_parts(kind, Some(q("x")), vec![], Attributes::new()).unwrap();
            assert!(r.attributes_mut().insert(q("label"), Value::from("x")));
            assert_eq!(r.attributes().iter().count(), 1, "{kind:?}");
        }
    }
}
