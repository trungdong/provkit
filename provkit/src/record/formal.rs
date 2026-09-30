//! Formal attributes: their names, their values and the list a record is
//! built from.

use super::{Kind, invalid};
use crate::{DateTime, QName, Result};

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

/// An owned formal attribute value, for [`Record::from_parts`](super::Record::from_parts).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Formal {
    /// A name-valued formal.
    Name(QName),
    /// A time-valued formal.
    Time(DateTime),
}

/// A borrowed formal attribute value, from [`Record::formals`](super::Record::formals).
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
pub(super) trait FormalValue {
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

/// Consumes a list of formals, checking each against the kind's table.
pub(super) struct Formals {
    items: Vec<(FormalAttr, Formal)>,
}

impl Formals {
    pub(super) fn new(kind: Kind, items: Vec<(FormalAttr, Formal)>) -> Result<Self> {
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
    pub(super) fn take<T: FormalValue>(&mut self, attr: FormalAttr) -> Option<T> {
        let i = self.items.iter().position(|(a, _)| *a == attr)?;
        Some(T::from_formal(self.items.remove(i).1))
    }
}
