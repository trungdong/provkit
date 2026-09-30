//! Formal attributes: their names, their values and the list a record is
//! built from.

use super::{Kind, invalid};
use crate::{DateTime, QName, Result};

/// Generates [`FormalAttr`] and its tables, and the `formal_type!` and
/// `formal_doc!` macros that the `records!` table consumes, from one list.
///
/// A row is `Variant: Type "localName"`, where the type is `QName` for a
/// name-valued formal and `DateTime` for a time-valued one.
macro_rules! formals {
    (@is_time DateTime) => { true };
    (@is_time QName) => { false };

    ($($name:ident: $ty:ident $local:literal,)*) => {
        /// A formal attribute name (PROV-DM's positional arguments).
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum FormalAttr {
            $(#[doc = concat!("`prov:", $local, "`.")] $name,)*
        }

        impl FormalAttr {
            /// Every formal attribute.
            pub const ALL: &'static [FormalAttr] = &[$(FormalAttr::$name),*];

            /// The local name under `prov:` (`entity`, `generatedEntity`, `startTime`, ...).
            pub fn local_name(self) -> &'static str {
                match self {
                    $(FormalAttr::$name => $local,)*
                }
            }

            /// Whether the value is a dateTime rather than a name.
            pub fn is_time(self) -> bool {
                match self {
                    $(FormalAttr::$name => formals!(@is_time $ty),)*
                }
            }
        }

        // The type a record's field holds for the formal.
        macro_rules! formal_type {
            $(($name) => { $ty };)*
        }

        // The doc line of a record's field for the formal.
        macro_rules! formal_doc {
            $(($name) => { concat!("`prov:", $local, "`.") };)*
        }
    };
}

formals! {
    Entity: QName "entity",
    Activity: QName "activity",
    Agent: QName "agent",
    Plan: QName "plan",
    Trigger: QName "trigger",
    Starter: QName "starter",
    Ender: QName "ender",
    Informed: QName "informed",
    Informant: QName "informant",
    Delegate: QName "delegate",
    Responsible: QName "responsible",
    Influencee: QName "influencee",
    Influencer: QName "influencer",
    GeneratedEntity: QName "generatedEntity",
    UsedEntity: QName "usedEntity",
    Generation: QName "generation",
    Usage: QName "usage",
    SpecificEntity: QName "specificEntity",
    GeneralEntity: QName "generalEntity",
    Alternate1: QName "alternate1",
    Alternate2: QName "alternate2",
    Collection: QName "collection",
    Bundle: QName "bundle",
    Time: DateTime "time",
    StartTime: DateTime "startTime",
    EndTime: DateTime "endTime",
}

impl FormalAttr {
    /// The formal for a `prov:` local name.
    pub fn from_local_name(name: &str) -> Option<FormalAttr> {
        FormalAttr::ALL
            .iter()
            .copied()
            .find(|f| f.local_name() == name)
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
    /// An owned [`Formal`] with a copy of the value.
    pub fn to_formal(self) -> Formal {
        match self {
            FormalRef::Name(q) => Formal::Name(q.clone()),
            FormalRef::Time(t) => Formal::Time(t.clone()),
        }
    }
}

impl From<FormalRef<'_>> for Formal {
    fn from(formal: FormalRef<'_>) -> Formal {
        formal.to_formal()
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
