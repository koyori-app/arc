//! 日付の入力を受ける規則。`start`・`end`・`today_iso` が同じ関数を通る。
//!
//! task の `hard_deadline` と `soft_deadline` は timestamptz なので、JSON では
//! `2026-06-01T09:00:00Z` のような日時の字で届く。日付だけを受ける作りだと、
//! 最初の実データで図が消える。そこで日時の字も受け、日付の部を取る。
//!
//! 時差の扱い: 字が持つ暦の日をそのまま取る。UTC へも利用者の時差へも換算しない。
//! `2026-06-08T23:30:00-05:00` は 6 月 8 日、`2026-06-08T09:00:00+09:00` も 6 月 8 日である。
//! 利用者の時差の日で描きたい呼び手は、渡す前にその時差で日付へ直す。

use chrono::{DateTime, NaiveDate, NaiveDateTime};
use serde::{de::Error as _, Deserialize, Deserializer};

/// 誤りの字を messages に載せる長さの上限（長い入力で表示が溢れぬように）。
const ECHO_MAX_CHARS: usize = 40;

/// `YYYY-MM-DD`、時差つきの日時（RFC 3339）、時差なしの日時（`T` か空白で区切る）を受け、日付を返す。
pub fn parse_date_or_datetime(input: &str) -> Result<NaiveDate, String> {
    let s = input.trim();
    if let Ok(date) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Ok(date);
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Ok(dt.date_naive());
    }
    for format in ["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%d %H:%M:%S%.f"] {
        if let Ok(dt) = NaiveDateTime::parse_from_str(s, format) {
            return Ok(dt.date());
        }
    }
    let shown: String = input.chars().take(ECHO_MAX_CHARS).collect();
    let ellipsis = if input.chars().count() > ECHO_MAX_CHARS {
        "…"
    } else {
        ""
    };
    Err(format!(
        "invalid date \"{shown}{ellipsis}\" (expected YYYY-MM-DD or an ISO 8601 date-time)"
    ))
}

/// `#[serde(deserialize_with = "...")]` 用。`GanttTask::start`。
pub fn deserialize_date<'de, D: Deserializer<'de>>(d: D) -> Result<NaiveDate, D::Error> {
    let s = String::deserialize(d)?;
    parse_date_or_datetime(&s).map_err(D::Error::custom)
}

/// `#[serde(deserialize_with = "...")]` 用。`GanttTask::end`。`null` と欠けは `None`。
pub fn deserialize_optional_date<'de, D: Deserializer<'de>>(
    d: D,
) -> Result<Option<NaiveDate>, D::Error> {
    match Option::<String>::deserialize(d)? {
        None => Ok(None),
        Some(s) => parse_date_or_datetime(&s)
            .map(Some)
            .map_err(D::Error::custom),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn plain_date_is_unchanged() {
        assert_eq!(parse_date_or_datetime("2026-06-08"), Ok(d(2026, 6, 8)));
        assert_eq!(parse_date_or_datetime(" 2026-06-08 "), Ok(d(2026, 6, 8)));
    }

    #[test]
    fn datetime_with_offset_keeps_the_calendar_day_it_is_written_in() {
        assert_eq!(
            parse_date_or_datetime("2026-06-01T09:00:00Z"),
            Ok(d(2026, 6, 1))
        );
        assert_eq!(
            parse_date_or_datetime("2026-06-08T09:00:00+09:00"),
            Ok(d(2026, 6, 8))
        );
        // UTC では 6 月 9 日の 04:30 だが、字は 6 月 8 日の 23:30 なので 8 日。
        assert_eq!(
            parse_date_or_datetime("2026-06-08T23:30:00-05:00"),
            Ok(d(2026, 6, 8))
        );
        // 日付の境のすぐ後。UTC に直すと前日になる字でも、書かれた日を取る。
        assert_eq!(
            parse_date_or_datetime("2026-06-09T00:30:00+09:00"),
            Ok(d(2026, 6, 9))
        );
    }

    #[test]
    fn datetime_with_fraction_and_without_offset() {
        assert_eq!(
            parse_date_or_datetime("2026-06-01T09:00:00.123Z"),
            Ok(d(2026, 6, 1))
        );
        assert_eq!(
            parse_date_or_datetime("2026-06-01T09:00:00"),
            Ok(d(2026, 6, 1))
        );
        assert_eq!(
            parse_date_or_datetime("2026-06-01 09:00:00"),
            Ok(d(2026, 6, 1))
        );
    }

    #[test]
    fn unreadable_input_is_an_error_not_a_guess() {
        for bad in [
            "",
            "garbage",
            "20260608",
            "2026-13-45",
            "2026-06-08T",
            "6/8",
        ] {
            assert!(
                parse_date_or_datetime(bad).is_err(),
                "{bad:?} must be refused"
            );
        }
    }

    #[test]
    fn error_message_is_bounded() {
        let long = "x".repeat(500);
        let msg = parse_date_or_datetime(&long).unwrap_err();
        assert!(msg.chars().count() < 160, "{}", msg.chars().count());
        assert!(msg.contains('…'));
    }
}
