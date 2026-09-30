//! `xsd:dateTime`.

use std::cmp::Ordering;
use std::fmt::{self, Write as _};
use std::hash::{Hash, Hasher};
use std::str::FromStr;

use crate::{Error, Result};

/// An `xsd:dateTime` value.
///
/// The value space follows XML Schema: a time with an offset denotes an
/// instant, a time without one denotes local wall-clock time, and the two
/// never compare equal. The text a value was parsed from is kept and printed
/// back; a value built from components prints in canonical form.
///
/// Years follow XSD 1.1: year 0000 exists, `-0000` is the same year, and year 0
/// is a leap year. Years are proleptic Gregorian in astronomical numbering.
///
/// Fractional seconds are held to nanoseconds. Digits beyond the ninth are
/// truncated for equality, ordering and hashing, and kept in the source text.
///
/// `DateTime` implements [`PartialOrd`] and not `Ord`. A zoned time and an
/// unzoned time have no defined order, so [`PartialOrd::partial_cmp`] returns
/// `None` for that pair and a total order does not exist.
#[derive(Clone, Debug)]
pub struct DateTime {
    year: i64,
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
    second: u8,
    nanos: u32,
    /// Offset from UTC in minutes; `None` for an unzoned time.
    offset: Option<i32>,
    source: Option<Box<str>>,
}

impl DateTime {
    /// Build from components. `hour` may be 24 only with zero minutes and
    /// seconds. `offset` is minutes from UTC, within ±14:00.
    ///
    /// The canonical text of a built value always parses back to an equal
    /// value.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidLiteral`] when a component is out of range, when
    /// the day does not exist in the month, or when the year is `i64::MIN` or
    /// the hour-24 rollover moves past `i64::MAX`. The error's `lexical` is the
    /// text of the rejected components.
    // One argument per XSD component; a parameter struct would only restate them.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        year: i64,
        month: u8,
        day: u8,
        hour: u8,
        minute: u8,
        second: u8,
        nanos: u32,
        offset: Option<i32>,
    ) -> Result<Self> {
        let dt = DateTime {
            year,
            month,
            day,
            hour,
            minute,
            second,
            nanos,
            offset,
            source: None,
        };
        dt.validate()
            .and_then(|()| dt.check_prints_back())
            .map_err(|reason| invalid(&dt.text(false), reason))?;
        Ok(dt)
    }

    /// Parse the XSD lexical form
    /// `-?YYYY(Y*)-MM-DDThh:mm:ss(.s+)?(Z|(+|-)hh:mm)?`.
    ///
    /// A year of more than four digits has no leading zero, and `0000` and
    /// `-0000` both denote year 0.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidLiteral`] when `text` does not match the form
    /// or a component is out of range.
    pub fn parse(text: &str) -> Result<Self> {
        let (date, rest) = text
            .split_once('T')
            .ok_or_else(|| invalid(text, "missing 'T'"))?;
        let (negative, date) = match date.strip_prefix('-') {
            Some(d) => (true, d),
            None => (false, date),
        };
        let mut parts = date.splitn(3, '-');
        let year_text = parts.next().unwrap_or("");
        let month_text = parts.next().ok_or_else(|| invalid(text, "missing month"))?;
        let day_text = parts.next().ok_or_else(|| invalid(text, "missing day"))?;
        if year_text.len() < 4 || !year_text.bytes().all(|b| b.is_ascii_digit()) {
            return Err(invalid(text, "year needs at least four digits"));
        }
        if year_text.len() > 4 && year_text.starts_with('0') {
            return Err(invalid(text, "year has a leading zero"));
        }
        let year: i64 = year_text
            .parse()
            .map_err(|_| invalid(text, "year out of range"))?;
        let year = if negative { -year } else { year };
        let month =
            two_digits(month_text).ok_or_else(|| invalid(text, "month is not two digits"))?;
        let day = two_digits(day_text).ok_or_else(|| invalid(text, "day is not two digits"))?;

        // Time: hh:mm:ss, optional fraction, optional zone.
        let (time, zone) = split_zone(rest);
        let (hms, fraction) = match time.split_once('.') {
            Some((h, f)) => (h, Some(f)),
            None => (time, None),
        };
        let mut t = hms.splitn(3, ':');
        let hour = two_digits(t.next().unwrap_or(""))
            .ok_or_else(|| invalid(text, "hour is not two digits"))?;
        let minute = two_digits(t.next().unwrap_or(""))
            .ok_or_else(|| invalid(text, "minute is not two digits"))?;
        let second = two_digits(t.next().unwrap_or(""))
            .ok_or_else(|| invalid(text, "second is not two digits"))?;
        let nanos = match fraction {
            Some(f) if !f.is_empty() && f.bytes().all(|b| b.is_ascii_digit()) => {
                // Keep nine digits; further digits are preserved in the source text only.
                let digits: String = f.chars().take(9).collect();
                let padded = format!("{digits:0<9}");
                padded
                    .parse::<u32>()
                    .map_err(|_| invalid(text, "bad fraction"))?
            }
            Some(_) => return Err(invalid(text, "fraction is not digits")),
            None => 0,
        };
        let offset = match zone {
            "" => None,
            "Z" => Some(0),
            z => {
                let sign = match z.as_bytes()[0] {
                    b'+' => 1,
                    b'-' => -1,
                    _ => return Err(invalid(text, "bad timezone")),
                };
                let (oh, om) = z[1..]
                    .split_once(':')
                    .ok_or_else(|| invalid(text, "bad timezone"))?;
                let oh = two_digits(oh).ok_or_else(|| invalid(text, "bad timezone hour"))?;
                let om = two_digits(om).ok_or_else(|| invalid(text, "bad timezone minute"))?;
                if oh > 14 || om > 59 || (oh == 14 && om != 0) {
                    return Err(invalid(text, "timezone outside -14:00..+14:00"));
                }
                Some(sign * (i32::from(oh) * 60 + i32::from(om)))
            }
        };
        let dt = DateTime {
            year,
            month,
            day,
            hour,
            minute,
            second,
            nanos,
            offset,
            source: Some(text.into()),
        };
        dt.validate().map_err(|reason| invalid(text, reason))?;
        Ok(dt)
    }

    fn validate(&self) -> std::result::Result<(), &'static str> {
        if !(1..=12).contains(&self.month) {
            return Err("month out of range");
        }
        if self.day == 0 || self.day > days_in_month(self.year, self.month) {
            return Err("day out of range");
        }
        if self.hour > 24 || self.minute > 59 || self.second > 59 {
            return Err("time out of range");
        }
        if self.hour == 24 && (self.minute != 0 || self.second != 0 || self.nanos != 0) {
            return Err("hour 24 needs 00:00:00");
        }
        if self.nanos >= 1_000_000_000 {
            return Err("fraction out of range");
        }
        if self.offset.is_some_and(|o| !(-840..=840).contains(&o)) {
            return Err("offset outside -14:00..+14:00");
        }
        Ok(())
    }

    /// Fails when the canonical text would not parse back: `i64::MIN` has no
    /// readable magnitude, and the hour-24 rollover can pass `i64::MAX`.
    fn check_prints_back(&self) -> std::result::Result<(), &'static str> {
        if self.year == i64::MIN {
            return Err("year out of range");
        }
        if i64::try_from(self.canonical_date().0).is_err() {
            return Err("year out of range after hour 24 rolls over");
        }
        Ok(())
    }

    /// The date and hour of the canonical form, where hour 24 becomes the next
    /// day's midnight.
    fn canonical_date(&self) -> (i128, i128, i128, i128) {
        if self.hour == 24 {
            let (y, m, d) = civil_from_days(days_from_civil(self.year, self.month, self.day) + 1);
            (y, m, d, 0)
        } else {
            (
                i128::from(self.year),
                i128::from(self.month),
                i128::from(self.day),
                i128::from(self.hour),
            )
        }
    }

    /// The components in canonical layout, without validation. `roll` moves hour
    /// 24 to the next day's midnight.
    fn text(&self, roll: bool) -> String {
        let (year, month, day, hour) = if roll {
            self.canonical_date()
        } else {
            (
                i128::from(self.year),
                i128::from(self.month),
                i128::from(self.day),
                i128::from(self.hour),
            )
        };
        let mut out = String::new();
        if year < 0 {
            let _ = write!(out, "-{:04}", year.unsigned_abs());
        } else {
            let _ = write!(out, "{year:04}");
        }
        let _ = write!(
            out,
            "-{month:02}-{day:02}T{hour:02}:{:02}:{:02}",
            self.minute, self.second
        );
        if self.nanos != 0 {
            let digits = format!("{:09}", self.nanos);
            let _ = write!(out, ".{}", digits.trim_end_matches('0'));
        }
        match self.offset {
            None => {}
            Some(0) => out.push('Z'),
            Some(o) => {
                let sign = if o < 0 { '-' } else { '+' };
                let minutes = o.unsigned_abs();
                let _ = write!(out, "{sign}{:02}:{:02}", minutes / 60, minutes % 60);
            }
        }
        out
    }

    /// The year, as written. `0000` and `-0000` both give 0.
    pub fn year(&self) -> i64 {
        self.year
    }

    /// The month, 1 to 12.
    pub fn month(&self) -> u8 {
        self.month
    }

    /// The day of the month, 1 to 31.
    pub fn day(&self) -> u8 {
        self.day
    }

    /// The hour, 0 to 24, as written. `24:00:00` gives 24.
    pub fn hour(&self) -> u8 {
        self.hour
    }

    /// The minute, 0 to 59.
    pub fn minute(&self) -> u8 {
        self.minute
    }

    /// The whole second, 0 to 59.
    pub fn second(&self) -> u8 {
        self.second
    }

    /// The fraction of a second in nanoseconds. Digits beyond the ninth are not
    /// held.
    pub fn nanosecond(&self) -> u32 {
        self.nanos
    }

    /// The offset from UTC in minutes, or `None` for an unzoned time.
    pub fn offset_minutes(&self) -> Option<i32> {
        self.offset
    }

    /// Whether the value carries a timezone offset.
    pub fn is_zoned(&self) -> bool {
        self.offset.is_some()
    }

    /// The comparison key, wide enough for any year `parse` accepts: seconds
    /// since 1970-01-01T00:00:00 (UTC when zoned, local wall-clock when not)
    /// and nanoseconds. Hour 24 rolls over here.
    fn key(&self) -> (i128, u32) {
        let days = days_from_civil(self.year, self.month, self.day);
        let secs = days * 86_400
            + i128::from(self.hour) * 3_600
            + i128::from(self.minute) * 60
            + i128::from(self.second)
            - i128::from(self.offset.unwrap_or(0)) * 60;
        (secs, self.nanos)
    }
}

impl PartialEq for DateTime {
    fn eq(&self, other: &Self) -> bool {
        self.is_zoned() == other.is_zoned() && self.key() == other.key()
    }
}

impl Eq for DateTime {}

impl Hash for DateTime {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.is_zoned().hash(state);
        self.key().hash(state);
    }
}

impl PartialOrd for DateTime {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        (self.is_zoned() == other.is_zoned()).then(|| self.key().cmp(&other.key()))
    }
}

impl fmt::Display for DateTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(source) = &self.source {
            return f.write_str(source);
        }
        // Canonical form: hour 24 becomes the next day's midnight.
        f.write_str(&self.text(true))
    }
}

impl FromStr for DateTime {
    type Err = Error;

    /// Calls [`DateTime::parse`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidLiteral`] under the conditions of
    /// [`DateTime::parse`].
    fn from_str(text: &str) -> Result<Self> {
        DateTime::parse(text)
    }
}

fn invalid(text: &str, reason: &str) -> Error {
    Error::InvalidLiteral {
        datatype: "xsd:dateTime".into(),
        lexical: text.into(),
        reason: reason.into(),
    }
}

fn two_digits(s: &str) -> Option<u8> {
    (s.len() == 2 && s.bytes().all(|b| b.is_ascii_digit()))
        .then(|| s.parse().ok())
        .flatten()
}

fn split_zone(time: &str) -> (&str, &str) {
    if let Some(t) = time.strip_suffix('Z') {
        return (t, "Z");
    }
    // The zone starts at the last '+' or '-' after the 'T' part, if any.
    match time.rfind(['+', '-']) {
        Some(i) if i > 0 => time.split_at(i),
        _ => (time, ""),
    }
}

fn is_leap(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_in_month(year: i64, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ => {
            if is_leap(year) {
                29
            } else {
                28
            }
        }
    }
}

/// Days since 1970-01-01 for a proleptic Gregorian date (Howard Hinnant's
/// `days_from_civil`, <http://howardhinnant.github.io/date_algorithms.html>).
///
/// Works in `i128` so no `i64` year can overflow.
fn days_from_civil(y: i64, m: u8, d: u8) -> i128 {
    let (m, d) = (i128::from(m), i128::from(d));
    let y = i128::from(y);
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The inverse of [`days_from_civil`], as `(year, month, day)`.
fn civil_from_days(z: i128) -> (i128, i128, i128) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::hash;

    #[test]
    fn parses_and_keeps_source() {
        for text in [
            "2011-11-16T16:05:33",
            "2011-11-16T16:05:33.265+01:00",
            "2011-11-16T16:05:33Z",
            "-0001-01-01T00:00:00",
            "2011-11-16T24:00:00",
            "12345-01-01T00:00:00.123456789Z",
        ] {
            let d = DateTime::parse(text).unwrap();
            assert_eq!(d.to_string(), text);
        }
    }

    #[test]
    fn fractions_beyond_nine_digits_truncate() {
        let long = DateTime::parse("2011-11-16T16:05:33.1234567899Z").unwrap();
        let nine = DateTime::parse("2011-11-16T16:05:33.123456789Z").unwrap();
        assert_eq!(long, nine);
        assert_eq!(hash(&long), hash(&nine));
        assert_eq!(long.to_string(), "2011-11-16T16:05:33.1234567899Z");
        assert_ne!(
            nine,
            DateTime::parse("2011-11-16T16:05:33.123456788Z").unwrap()
        );
    }

    #[test]
    fn rejects_invalid() {
        for text in [
            "2011-13-01T00:00:00",
            "2011-02-29T00:00:00",
            "2011-11-16T24:00:01",
            "2011-11-16 16:05:33",
            "2011-11-16T16:05:33+15:00",
            "not a date",
            "",
        ] {
            assert!(
                matches!(DateTime::parse(text), Err(Error::InvalidLiteral { .. })),
                "{text}"
            );
        }
    }

    #[test]
    fn xsd_equality() {
        let a = DateTime::parse("2011-11-16T13:00:00+01:00").unwrap();
        let b = DateTime::parse("2011-11-16T12:00:00Z").unwrap();
        let c = DateTime::parse("2011-11-16T12:00:00").unwrap();
        let d = DateTime::parse("2011-11-16T12:00:00.000").unwrap();
        assert_eq!(a, b);
        assert_eq!(hash(&a), hash(&b));
        assert_ne!(b, c);
        assert_eq!(c, d);
        assert_eq!(hash(&c), hash(&d));
        assert_eq!(a.partial_cmp(&c), None);
        assert!(a < DateTime::parse("2011-11-16T12:00:01Z").unwrap());
    }

    #[test]
    fn hour_24_is_next_midnight() {
        let a = DateTime::parse("2011-11-16T24:00:00Z").unwrap();
        let b = DateTime::parse("2011-11-17T00:00:00Z").unwrap();
        assert_eq!(a, b);
        assert_eq!(hash(&a), hash(&b));
        assert_eq!(a.to_string(), "2011-11-16T24:00:00Z");

        let a = DateTime::parse("2011-11-16T24:00:00").unwrap();
        let b = DateTime::parse("2011-11-17T00:00:00").unwrap();
        assert_eq!(a, b);
        assert_eq!(hash(&a), hash(&b));

        let a = DateTime::parse("2011-11-16T24:00:00-05:00").unwrap();
        let b = DateTime::parse("2011-11-17T05:00:00Z").unwrap();
        assert_eq!(a, b);
        assert_eq!(hash(&a), hash(&b));
    }

    #[test]
    fn new_rejects_offsets_out_of_range() {
        for offset in [841, -841, 100_000, i32::MIN, i32::MAX] {
            assert!(
                matches!(
                    DateTime::new(2011, 11, 16, 0, 0, 0, 0, Some(offset)),
                    Err(Error::InvalidLiteral { .. })
                ),
                "{offset}"
            );
        }
        assert!(DateTime::new(2011, 11, 16, 0, 0, 0, 0, Some(840)).is_ok());
        assert!(DateTime::new(2011, 11, 16, 0, 0, 0, 0, Some(-840)).is_ok());
    }

    #[test]
    fn canonical_form_when_built() {
        let d = DateTime::new(2011, 11, 16, 16, 5, 33, 265_000_000, Some(0)).unwrap();
        assert_eq!(d.to_string(), "2011-11-16T16:05:33.265Z");
        let d = DateTime::new(2011, 11, 16, 24, 0, 0, 0, None).unwrap();
        assert_eq!(d.to_string(), "2011-11-17T00:00:00");
    }

    #[test]
    fn huge_years_do_not_panic() {
        let a = DateTime::parse("999999999999-01-01T00:00:00Z").unwrap();
        let b = DateTime::parse("999999999999-01-01T00:00:00Z").unwrap();
        assert_eq!(a, b);
        assert_eq!(hash(&a), hash(&b));
        let max = DateTime::parse("9223372036854775807-12-31T24:00:00-14:00").unwrap();
        assert_eq!(max, max.clone());
        assert!(max > a);
        let min = DateTime::parse("-9223372036854775807-01-01T00:00:00").unwrap();
        assert_eq!(hash(&min), hash(&min.clone()));
        let max_year = DateTime::parse("9223372036854775807-01-01T00:00:00").unwrap();
        assert_eq!(hash(&max_year), hash(&max_year.clone()));
        assert!(min < max_year);
        assert!(max_year > min);
        assert!(DateTime::new(i64::MAX, 12, 31, 24, 0, 0, 0, None).is_err());
    }

    #[test]
    fn year_zero_exists_and_is_a_leap_year() {
        let zero = DateTime::parse("0000-02-29T00:00:00Z").unwrap();
        assert_eq!(zero.year(), 0);
        assert_eq!(DateTime::parse("-0000-02-29T00:00:00Z").unwrap(), zero);
        assert!(DateTime::parse("-0001-02-29T00:00:00Z").is_err());
        assert!(
            DateTime::parse("0000-01-01T00:00:00").unwrap()
                < DateTime::parse("0001-01-01T00:00:00").unwrap()
        );
        assert_eq!(
            DateTime::parse("0000-01-01T00:00:00").unwrap(),
            DateTime::parse("-0000-01-01T00:00:00").unwrap()
        );
    }

    #[test]
    fn timeline_is_continuous_across_year_zero() {
        let before = DateTime::parse("-0001-12-31T23:59:59Z").unwrap();
        let start = DateTime::parse("0000-01-01T00:00:00Z").unwrap();
        assert!(before < start);
        let diff = |a: &DateTime, b: &DateTime| a.key().0 - b.key().0;
        assert_eq!(diff(&start, &before), 1);
        assert_eq!(
            DateTime::parse("0000-12-31T24:00:00Z").unwrap(),
            DateTime::parse("0001-01-01T00:00:00Z").unwrap()
        );
        assert_eq!(
            diff(&DateTime::parse("0001-01-01T00:00:00Z").unwrap(), &start),
            366 * 86_400
        );
    }

    #[test]
    fn year_text_follows_the_xsd_1_1_year_frag() {
        for text in [
            "00000-01-01T00:00:00",
            "02012-01-01T00:00:00",
            "-00000-01-01T00:00:00",
            "012-01-01T00:00:00",
        ] {
            assert!(
                matches!(DateTime::parse(text), Err(Error::InvalidLiteral { .. })),
                "{text}"
            );
        }
        assert!(DateTime::parse("10000-01-01T00:00:00").is_ok());
    }

    #[test]
    fn whatever_new_accepts_prints_and_parses_back() {
        for year in [
            i64::MIN,
            i64::MIN + 1,
            -1,
            0,
            1,
            9999,
            10_000,
            i64::MAX - 1,
            i64::MAX,
        ] {
            for (month, day, hour) in [(1, 1, 0), (12, 31, 23), (12, 31, 24)] {
                for offset in [None, Some(0), Some(-840), Some(330)] {
                    if let Ok(value) = DateTime::new(year, month, day, hour, 0, 0, 7, offset) {
                        let back: DateTime = value.to_string().parse().unwrap();
                        assert_eq!(back, value, "{value}");
                    }
                }
            }
        }
        assert!(DateTime::new(i64::MAX, 12, 31, 24, 0, 0, 0, None).is_err());
        assert!(DateTime::new(i64::MIN, 1, 1, 0, 0, 0, 0, None).is_err());
        assert!(DateTime::new(i64::MAX, 12, 31, 23, 0, 0, 0, None).is_ok());
    }

    #[test]
    fn new_errors_name_the_components() {
        let lexical = |result: Result<DateTime>| match result {
            Err(Error::InvalidLiteral { lexical, .. }) => lexical,
            other => panic!("expected InvalidLiteral, got {other:?}"),
        };
        assert_eq!(
            lexical(DateTime::new(2012, 13, 1, 0, 0, 0, 0, None)),
            "2012-13-01T00:00:00"
        );
        assert_eq!(
            lexical(DateTime::new(-5, 2, 30, 1, 2, 3, 500_000_000, Some(-90))),
            "-0005-02-30T01:02:03.5-01:30"
        );
        assert_eq!(
            lexical(DateTime::new(2012, 1, 1, 0, 0, 0, 0, Some(i32::MIN))),
            "2012-01-01T00:00:00-35791394:08"
        );
        assert_eq!(
            lexical(DateTime::new(i64::MAX, 12, 31, 24, 0, 0, 0, Some(0))),
            "9223372036854775807-12-31T24:00:00Z"
        );
    }

    #[test]
    fn accessors_return_components_as_written() {
        let d = DateTime::parse("-0000-02-29T24:00:00-05:30").unwrap();
        assert_eq!(
            (
                d.year(),
                d.month(),
                d.day(),
                d.hour(),
                d.minute(),
                d.second()
            ),
            (0, 2, 29, 24, 0, 0)
        );
        assert_eq!(d.nanosecond(), 0);
        assert_eq!(d.offset_minutes(), Some(-330));
        assert_eq!(
            DateTime::parse("2011-11-16T16:05:33")
                .unwrap()
                .offset_minutes(),
            None
        );
        assert_eq!(
            DateTime::parse("2011-11-16T16:05:33Z")
                .unwrap()
                .offset_minutes(),
            Some(0)
        );
    }

    #[test]
    fn from_str_parses() {
        let d: DateTime = "2011-11-16T16:05:33Z".parse().unwrap();
        assert_eq!(d, DateTime::parse("2011-11-16T16:05:33Z").unwrap());
        assert!("nope".parse::<DateTime>().is_err());
    }
}
