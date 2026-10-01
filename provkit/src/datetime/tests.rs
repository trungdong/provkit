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
        "+2012-01-01T00:00:00",
        "2011-11-16T16:05:33+15:00",
        "2011-00-01T00:00:00",
        "2011-01-00T00:00:00",
        "2011-04-31T00:00:00",
        "2011-06-31T00:00:00",
        "2011-11-16T25:00:00",
        "2011-11-16T16:60:00",
        "2011-11-16T16:05:60",
        "2011-11-16T24:30:00",
        "2011-11-16T24:00:00.5",
        "2011-11-16T24:00:00.0000000001",
        "2011-11-16T16:05:33.",
        "2011-11-16T16:05:33.x",
        "2011-11-16T16:05:33.Z",
        "2011-11-16T16:05:33+10:60",
        "2011-1-01T00:00:00",
        "2011-01-1T00:00:00",
        "2011-01-01T1:00:00",
        "2011-01-01T01:2:00",
        "2011-01-01T01:02:3",
        "2011-01-01T01:02:03+1:00",
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
fn hour_24_takes_a_zero_fraction_of_any_length() {
    let midnight = DateTime::parse("2011-11-17T00:00:00").unwrap();
    for text in [
        "2011-11-16T24:00:00",
        "2011-11-16T24:00:00.0",
        "2011-11-16T24:00:00.000000000000",
    ] {
        assert_eq!(DateTime::parse(text).unwrap(), midnight, "{text}");
    }
}

#[test]
fn new_rejects_each_component_out_of_range() {
    let new = |minute, second, nanos| DateTime::new(2011, 1, 1, 0, minute, second, nanos, None);
    assert!(new(59, 59, 999_999_999).is_ok());
    for (minute, second, nanos) in [(60, 0, 0), (0, 60, 0), (0, 0, 1_000_000_000)] {
        assert!(
            matches!(
                new(minute, second, nanos),
                Err(Error::InvalidLiteral { .. })
            ),
            "{minute} {second} {nanos}"
        );
    }
    let Err(Error::InvalidLiteral { lexical, .. }) = new(0, 0, 1_000_000_000) else {
        panic!("a fraction of a whole second is out of range");
    };
    assert!(lexical.contains("1000000000"), "{lexical}");
    assert!(!lexical.contains(".1"), "{lexical}");
}

#[test]
fn new_rejects_a_fraction_at_hour_24() {
    assert!(DateTime::new(2011, 1, 1, 24, 0, 0, 0, None).is_ok());
    assert!(matches!(
        DateTime::new(2011, 1, 1, 24, 0, 0, 1, None),
        Err(Error::InvalidLiteral { .. })
    ));
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

    let far_min = DateTime::parse("-9223372036854775807-01-01T00:00:00+14:00").unwrap();
    assert_eq!(hash(&far_min), hash(&far_min.clone()));
    assert!(far_min < max);
    assert!(max > far_min);
    assert_ne!(far_min, min);
}

#[test]
fn negative_years_follow_the_proleptic_leap_rule() {
    for text in ["-0004-02-29T00:00:00", "-0400-02-29T00:00:00"] {
        assert!(DateTime::parse(text).is_ok(), "{text}");
    }
    assert!(matches!(
        DateTime::parse("-0100-02-29T00:00:00"),
        Err(Error::InvalidLiteral { .. })
    ));
}

#[test]
fn year_zero_exists_and_is_a_leap_year() {
    let zero = DateTime::parse("0000-02-29T00:00:00Z").unwrap();
    assert_eq!(zero.year(), 0);
    assert_eq!(DateTime::parse("-0000-02-29T00:00:00Z").unwrap(), zero);
    assert_eq!(
        hash(&DateTime::parse("-0000-02-29T00:00:00Z").unwrap()),
        hash(&zero)
    );
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
