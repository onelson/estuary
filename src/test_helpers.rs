use crate::package_index::{Config, PackageIndex};
use crate::Settings;
use actix_web::web;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tempdir::TempDir;

/// This is the request body sent to the publish endpoint from an empty bin crate.
pub const MY_CRATE_0_1_0: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/test_data/publish-my-crate-body"
));

pub fn get_data_root() -> TempDir {
    TempDir::new("estuary_test").unwrap()
}

pub fn get_test_package_index(data_dir: &Path) -> web::Data<Mutex<PackageIndex>> {
    let config = Config {
        api: String::new(),
        dl: String::new(),
    };
    web::Data::new(Mutex::new(PackageIndex::init(data_dir, &config).unwrap()))
}

pub fn get_test_settings(data_dir: &Path) -> web::Data<Settings> {
    let settings = Settings {
        crate_dir: data_dir.join("crates").to_path_buf(),
        index_dir: data_dir.join("index").to_path_buf(),
        git_binary: PathBuf::from("git"),
    };
    std::fs::create_dir_all(&settings.index_dir).unwrap();
    std::fs::create_dir_all(&settings.crate_dir).unwrap();
    let conn = settings.get_db().unwrap();
    crate::database::init(&conn).unwrap();
    web::Data::new(settings)
}
