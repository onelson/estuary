use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

pub fn get_crate_file_path<P: AsRef<Path>>(
    root: P,
    name: &str,
    vers: &str,
) -> anyhow::Result<PathBuf> {
    let dir = root.as_ref().join(name);
    let fp = dir.join(&format!("{}-{}.crate", name, vers));
    Ok(fp)
}

/// Write bytes to crate storage.
pub fn store_crate_file<P: AsRef<Path>>(
    root: P,
    name: &str,
    vers: &str,
    content: &[u8],
) -> anyhow::Result<()> {
    let fp = get_crate_file_path(root.as_ref(), name, vers)?;
    fs::create_dir_all(fp.parent().unwrap())?;

    let mut fh = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(fp)?;
    fh.write_all(content)?;
    Ok(())
}
