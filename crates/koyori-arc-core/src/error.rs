//! Machine-readable error codes shared with the JS bindings.
//!
//! Callers branch on `code`; `error` is a human-readable message that may be
//! reworded at any time without a version bump. The Vue package mirrors these
//! literals in `packages/arc-vue/src/wasmContract.ts` and its contract test
//! reads *this file* back, so changing a value here without changing the mirror
//! fails that test.

/// The Canvas backing store cannot hold this chart, but the independently
/// bounded SVG renderer still can — JS may fall back instead of erroring.
pub const CODE_CANVAS_CAPACITY: &str = "canvas_capacity";
/// The input exceeds a limit both renderers share, or carries dates the
/// renderer refuses. No fallback will succeed.
pub const CODE_INPUT_LIMIT: &str = "input_limit";
/// The tasks or dependencies JSON could not be deserialized.
pub const CODE_PARSE_ERROR: &str = "parse_error";
/// The command buffer could not be serialized back to JSON.
pub const CODE_SERIALIZE_ERROR: &str = "serialize_error";

/// A refusal from a Wasm entry point: a stable `code` for control flow and a
/// message for display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderError {
    pub code: &'static str,
    pub message: String,
}

impl RenderError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    /// `{"error":"…","code":"…"}` — the only shape JS parses.
    pub fn to_json(&self) -> String {
        serde_json::json!({ "error": self.message, "code": self.code }).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_carries_both_code_and_message() {
        let err = RenderError::new(CODE_PARSE_ERROR, r#"parse error: bad "quote""#);
        let v: serde_json::Value = serde_json::from_str(&err.to_json()).expect("valid json");
        assert_eq!(v["code"].as_str(), Some("parse_error"));
        assert_eq!(v["error"].as_str(), Some(r#"parse error: bad "quote""#));
    }
}
