use crate::{error::require, Result};
use std::{
    fs,
    os::unix::fs::DirBuilderExt,
    path::{Path, PathBuf},
};
pub fn secure_dir(path: &Path) -> Result<()> {
    require(
        path.is_absolute(),
        "catalog_path",
        "An absolute catalog directory is required",
        400,
    )?;
    let mut check = PathBuf::from("/");
    for c in path.components() {
        check.push(c);
        if let Ok(m) = check.symlink_metadata() {
            require(
                !m.file_type().is_symlink(),
                "catalog_path",
                "Catalog directory redirected",
                409,
            )?;
        }
    }
    if !path.exists() {
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(path)?;
    }
    require(
        path.canonicalize()? == path,
        "catalog_path",
        "Catalog directory redirected",
        409,
    )?;
    Ok(())
}
