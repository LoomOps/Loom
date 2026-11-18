use crate::{error::require, storage, Result};
use std::path::Path;
pub fn bounded(path: &Path, max: usize) -> Result<Vec<u8>> {
    use std::io::Read;
    let mut bytes = vec![];
    storage::open_regular(path)?
        .take(max as u64 + 1)
        .read_to_end(&mut bytes)?;
    require(
        bytes.len() <= max,
        "inventory",
        "Inventory metadata exceeds bound",
        413,
    )?;
    Ok(bytes)
}
