//! 日付の入力を受ける規則。`start`・`end`・`today_iso` が同じ関数を通る。
//!
//! task の `hard_deadline` と `soft_deadline` は timestamptz なので、JSON では
//! `2026-06-01T09:00:00Z` のような日時の字で届く。日付だけを受ける作りだと、
//! 最初の実データで図が消える。そこで日時の字も受け、日付の部を取る。
//!
//! どの暦の日を取るかは、時差の指定（`tz_offset_minutes`）で決まる。
//!
//! - 指定なし: 字が書いている暦の日をそのまま取る。`2026-06-09T00:30:00+09:00` は 6 月 9 日。換算しない。
//! - 指定あり: 時差つきの日時の字を、その時差の暦の日へ直す。
//!   `2026-06-08T15:30:00Z` は、540（JST）なら 6 月 9 日、0 なら 6 月 8 日。
//! - 日付だけの字（`2026-06-08`）は、指定の有無によらず換算しない。暦の日がもう書かれていて、
//!   換算すると逆にずれる。
//! - 時差のない日時の字（`2026-06-08T09:00:00`）も換算しない。どの時差の時刻か分からず、直す元が無い。
//!   書かれた日を取る。
//!
//! 指定なしと `0` は同じではない。`+09:00` の字は、指定なしなら書かれた日、`0` なら UTC の日になる。

use chrono::{DateTime, FixedOffset, NaiveDate, NaiveDateTime};
use serde::{de::Error as _, Deserialize, Deserializer};

/// 時差の分（符号つき）の上限。これを含めて越える値は正気でないとして断る。
/// 現実の時差は -720 から 840。`FixedOffset` が受けるのは 24 時間未満である。
pub const MAX_TZ_OFFSET_MINUTES: f64 = 1439.0;

/// JS から来る `tz_offset_minutes` を検める。`None`（指定なし）は `None` のまま返す。
/// 整数でない値、NaN、無限、範囲外は断る。`i32` で受けると NaN が黙って 0 になるので、`f64` で受けて検める。
pub fn parse_tz_offset(minutes: Option<f64>) -> Result<Option<FixedOffset>, String> {
    let Some(m) = minutes else {
        return Ok(None);
    };
    if !m.is_finite() || m.fract() != 0.0 || m.abs() > MAX_TZ_OFFSET_MINUTES {
        return Err(format!(
            "invalid tz_offset_minutes {m} (expected an integer between -{MAX_TZ_OFFSET_MINUTES} and {MAX_TZ_OFFSET_MINUTES})"
        ));
    }
    FixedOffset::east_opt(m as i32 * 60)
        .map(Some)
        .ok_or_else(|| format!("invalid tz_offset_minutes {m}"))
}

/// 誤りの字を messages に載せる長さの上限（長い入力で表示が溢れぬように）。
const ECHO_MAX_CHARS: usize = 40;

/// `YYYY-MM-DD`、時差つきの日時（RFC 3339）、時差なしの日時（`T` か空白で区切る）を受け、日付を返す。
/// 時差の指定なし。字が書いている暦の日をそのまま取る。
pub fn parse_date_or_datetime(input: &str) -> Result<NaiveDate, String> {
    resolve_date(input, None)
}

/// `parse_date_or_datetime` に、利用者の時差を足したもの。
/// `offset` が `Some` の時だけ、時差つきの日時の字をその時差の暦の日へ直す。
pub fn resolve_date(input: &str, offset: Option<FixedOffset>) -> Result<NaiveDate, String> {
    let s = input.trim();
    if let Ok(date) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Ok(date);
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Ok(match offset {
            Some(tz) => dt.with_timezone(&tz).date_naive(),
            None => dt.date_naive(),
        });
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

    fn tz(minutes: f64) -> Option<FixedOffset> {
        parse_tz_offset(Some(minutes)).unwrap()
    }

    #[test]
    fn a_utc_string_moves_to_the_next_day_in_jst_and_stays_in_utc() {
        // 陽性対照。6 月 8 日 15:30 UTC は、日本では 6 月 9 日の 00:30。
        assert_eq!(
            resolve_date("2026-06-08T15:30:00Z", tz(540.0)),
            Ok(d(2026, 6, 9))
        );
        assert_eq!(
            resolve_date("2026-06-08T15:30:00Z", tz(0.0)),
            Ok(d(2026, 6, 8))
        );
    }

    #[test]
    fn a_date_only_string_is_never_converted() {
        for minutes in [-720.0, 0.0, 540.0, 840.0] {
            assert_eq!(
                resolve_date("2026-06-08", tz(minutes)),
                Ok(d(2026, 6, 8)),
                "{minutes}"
            );
        }
        assert_eq!(resolve_date("2026-06-08", None), Ok(d(2026, 6, 8)));
    }

    #[test]
    fn a_datetime_without_an_offset_is_never_converted() {
        for input in ["2026-06-08T23:30:00", "2026-06-08 23:30:00"] {
            assert_eq!(resolve_date(input, tz(540.0)), Ok(d(2026, 6, 8)), "{input}");
            assert_eq!(
                resolve_date(input, tz(-720.0)),
                Ok(d(2026, 6, 8)),
                "{input}"
            );
        }
    }

    #[test]
    fn strings_near_a_day_boundary() {
        // JST の 6 月 8 日 23:30 は UTC では 14:30。JST で見れば 8 日、UTC で見ても 8 日。
        assert_eq!(
            resolve_date("2026-06-08T23:30:00+09:00", tz(540.0)),
            Ok(d(2026, 6, 8))
        );
        assert_eq!(
            resolve_date("2026-06-08T23:30:00+09:00", tz(0.0)),
            Ok(d(2026, 6, 8))
        );
        // 6 月 9 日 00:30 UTC は、JST では 9 日、アメリカ東部（-300）では 8 日。
        assert_eq!(
            resolve_date("2026-06-09T00:30:00Z", tz(540.0)),
            Ok(d(2026, 6, 9))
        );
        assert_eq!(
            resolve_date("2026-06-09T00:30:00Z", tz(-300.0)),
            Ok(d(2026, 6, 8))
        );
        // JST の 6 月 9 日 00:30 は UTC では 8 日。指定 0 なら 8 日、指定なしなら書かれた 9 日。
        assert_eq!(
            resolve_date("2026-06-09T00:30:00+09:00", tz(0.0)),
            Ok(d(2026, 6, 8))
        );
        assert_eq!(
            resolve_date("2026-06-09T00:30:00+09:00", None),
            Ok(d(2026, 6, 9))
        );
    }

    #[test]
    fn no_offset_keeps_the_written_day_which_is_not_the_same_as_zero() {
        assert_eq!(
            parse_date_or_datetime("2026-06-09T00:30:00+09:00"),
            resolve_date("2026-06-09T00:30:00+09:00", None)
        );
        assert_ne!(
            resolve_date("2026-06-09T00:30:00+09:00", None),
            resolve_date("2026-06-09T00:30:00+09:00", tz(0.0))
        );
    }

    #[test]
    fn absurd_offsets_are_refused() {
        for bad in [1440.0, -1440.0, 100000.0, 30.5, f64::NAN, f64::INFINITY] {
            assert!(parse_tz_offset(Some(bad)).is_err(), "{bad} must be refused");
        }
        for ok in [-720.0, -1439.0, 0.0, 330.0, 540.0, 840.0, 1439.0] {
            assert!(parse_tz_offset(Some(ok)).is_ok(), "{ok} must be accepted");
        }
        assert_eq!(parse_tz_offset(None), Ok(None));
    }
}
