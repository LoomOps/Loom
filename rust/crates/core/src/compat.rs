//! Exact legacy JSON namespaces, independent of typed domain decisions.
use serde_json::Value;
fn string(value: &str, ascii: bool) -> String {
    let s = serde_json::to_string(value).expect("JSON string");
    if !ascii {
        return s;
    }
    let mut out = String::new();
    for ch in s.chars() {
        if ch.is_ascii() {
            out.push(ch)
        } else {
            let mut buf = [0; 2];
            for unit in ch.encode_utf16(&mut buf) {
                out.push_str(&format!("\\u{unit:04x}"));
            }
        }
    }
    out
}
pub fn json(value: &Value, compact: bool, ascii: bool) -> String {
    let comma = if compact { "," } else { ", " };
    let colon = if compact { ":" } else { ": " };
    match value {
        Value::Array(a) => format!(
            "[{}]",
            a.iter()
                .map(|v| json(v, compact, ascii))
                .collect::<Vec<_>>()
                .join(comma)
        ),
        Value::Object(o) => format!(
            "{{{}}}",
            o.iter()
                .map(|(k, v)| format!("{}{colon}{}", string(k, ascii), json(v, compact, ascii)))
                .collect::<Vec<_>>()
                .join(comma)
        ),
        Value::String(s) => string(s, ascii),
        Value::Number(n) => {
            let raw = n.to_string();
            if raw.contains(['.', 'e', 'E']) {
                if let Some(number) = n.as_f64().filter(|v| v.is_finite()) {
                    let s = format!("{number:?}");
                    if let Some((m, e)) = s.split_once('e') {
                        let e: i32 = e.parse().expect("float exponent");
                        format!("{m}e{}{:02}", if e < 0 { "-" } else { "+" }, e.abs())
                    } else {
                        s
                    }
                } else {
                    raw
                }
            } else if raw == "-0" {
                "0".into()
            } else {
                raw
            }
        }
        _ => value.to_string(),
    }
}
pub fn dump(v: &Value) -> String {
    json(v, false, true)
}
pub fn seal(v: &Value) -> String {
    json(v, true, true)
}
#[cfg(test)]
#[path = "../../../../tests/rust/codec.rs"]
mod tests;
