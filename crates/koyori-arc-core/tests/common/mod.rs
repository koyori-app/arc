//! 試験が共有する小さな道具。

/// 以前の位置の引数（`today_iso`、`viewport_json`、`tz_offset_minutes`）を、
/// 一つの `options_json` に直す。
///
/// 書き換える前の呼び出しと同じ値を、同じ意味で渡すための橋である。
/// 何も指定しない時は `None` を返す（省略は既定と同じ）。
/// JSON に書けない数（NaN と無限大）は字にして渡し、入口が断ることを確かめられるようにする。
#[allow(dead_code)]
pub fn options_json(
    today: Option<String>,
    viewport: Option<String>,
    tz_offset_minutes: Option<f64>,
) -> Option<String> {
    let mut map = serde_json::Map::new();
    if let Some(today) = today {
        map.insert("today".to_string(), serde_json::Value::String(today));
    }
    if let Some(viewport) = viewport {
        let value = serde_json::from_str(&viewport)
            .unwrap_or_else(|_| serde_json::Value::String(viewport.clone()));
        map.insert("scroll_viewport".to_string(), value);
    }
    if let Some(tz) = tz_offset_minutes {
        let value = serde_json::Number::from_f64(tz)
            .map(serde_json::Value::Number)
            .unwrap_or_else(|| serde_json::Value::String(tz.to_string()));
        map.insert("tz_offset_minutes".to_string(), value);
    }
    if map.is_empty() {
        None
    } else {
        Some(serde_json::Value::Object(map).to_string())
    }
}
