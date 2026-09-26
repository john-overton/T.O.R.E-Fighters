//! Hand-written JSON: correct string escaping, numbers rounded for reading,
//! and `null` for anything that is not a finite number.

use crate::model::Value;
use std::fmt::Write as _;

pub(crate) fn string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            // Line and paragraph separators break some JavaScript readers.
            c if (c as u32) < 0x20 || c == '\u{2028}' || c == '\u{2029}' => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A number rounded to `decimals`, or `null` when it is not finite.
pub(crate) fn number(v: f64, decimals: usize) -> String {
    if !v.is_finite() {
        return "null".into();
    }
    super::text::num(v, decimals)
}

/// A number with full precision (shortest form that reads back exactly).
pub(crate) fn exact(v: f64) -> String {
    if !v.is_finite() {
        return "null".into();
    }
    let text = format!("{v}");
    if text.len() > 24 {
        format!("{v:e}")
    } else {
        text
    }
}

pub(crate) fn numbers(values: &[f64], decimals: usize) -> String {
    let parts: Vec<String> = values.iter().map(|v| number(*v, decimals)).collect();
    format!("[{}]", parts.join(","))
}

pub(crate) fn value(v: &Value) -> String {
    match v {
        Value::None => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::Int(i) => i.to_string(),
        Value::Num(n) => exact(*n),
        Value::Text(t) => string(t),
        Value::Id(id) => id.to_string(),
        Value::Ids(ids) => format!(
            "[{}]",
            ids.iter().map(u32::to_string).collect::<Vec<_>>().join(",")
        ),
    }
}

/// Builds one JSON object, field by field, in order.
pub(crate) struct Object {
    text: String,
}

impl Object {
    pub fn new() -> Self {
        Self {
            text: String::from("{"),
        }
    }

    /// Adds a field whose value is already JSON.
    pub fn raw(mut self, key: &str, json: impl AsRef<str>) -> Self {
        if self.text.len() > 1 {
            self.text.push(',');
        }
        self.text.push_str(&string(key));
        self.text.push(':');
        self.text.push_str(json.as_ref());
        self
    }

    pub fn str(self, key: &str, text: &str) -> Self {
        self.raw(key, string(text))
    }

    pub fn int(self, key: &str, v: impl Into<i128>) -> Self {
        let v: i128 = v.into();
        self.raw(key, v.to_string())
    }

    pub fn num(self, key: &str, v: f64, decimals: usize) -> Self {
        self.raw(key, number(v, decimals))
    }

    pub fn bool(self, key: &str, v: bool) -> Self {
        self.raw(key, v.to_string())
    }

    pub fn finish(mut self) -> String {
        self.text.push('}');
        self.text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings_escape_every_control_character() {
        assert_eq!(
            string("a\"b\\c\n\u{1}\u{2028}é"),
            "\"a\\\"b\\\\c\\n\\u0001\\u2028é\""
        );
        assert_eq!(number(f64::NAN, 2), "null");
        assert_eq!(number(-0.0, 2), "0");
        assert_eq!(exact(0.1), "0.1");
        assert_eq!(exact(1e300), "1e300");
        let object = Object::new()
            .str("a", "x")
            .int("b", -3i64)
            .num("c", 1.26, 1)
            .finish();
        assert_eq!(object, "{\"a\":\"x\",\"b\":-3,\"c\":1.3}");
    }
}
