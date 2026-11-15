use super::*;
use serde_json::json;
#[test]
fn namespace_spacing_and_unicode_are_preserved() {
    let v = json!({"a":"가😀","b":-0.0,"c":1e-7});
    assert_eq!(
        dump(&v),
        "{\"a\": \"\\uac00\\ud83d\\ude00\", \"b\": -0.0, \"c\": 1e-07}"
    );
    assert!(!seal(&v).contains(": "));
}
