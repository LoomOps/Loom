use crate::Result;
use rusqlite::Connection;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
pub fn now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}
pub fn hash(bytes: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(bytes.as_ref()))
}
pub fn id() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}
pub fn encode(v: &Value) -> Result<String> {
    Ok(serde_json::to_string(v)?)
}
pub fn json_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    let mut result = serde_json::Map::new();
    for i in 0..row.as_ref().column_count() {
        let key = row.as_ref().column_name(i)?;
        let v = match row.get_ref(i)? {
            rusqlite::types::ValueRef::Null => Value::Null,
            rusqlite::types::ValueRef::Integer(n) => json!(n),
            rusqlite::types::ValueRef::Real(n) => json!(n),
            rusqlite::types::ValueRef::Text(t) => Value::String(String::from_utf8_lossy(t).into()),
            _ => Value::Null,
        };
        result.insert(key.into(), v);
    }
    Ok(Value::Object(result))
}
pub fn rows(db: &Connection, sql: &str, args: &[&dyn rusqlite::ToSql]) -> Result<Vec<Value>> {
    let mut stmt = db.prepare(sql)?;
    let items = stmt
        .query_map(args, json_row)?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(items)
}
