//! These endpoints relate to the core packaging features.
//!
//! Publish, yank, unyank, and download are the bare essentials needed for
//! adding new crates to the registry and using the registry to install crates.
//!
//! For now, the endpoints related to "owners" are on the back burner.
//! For the small-scale use case estuary targets, we may not need them at all.
//!
//! The search endpoint is still pending, but it's on the more near term list.
//!
//! - [x] Publish `PUT /api/v1/crates/new`.
//! - [x] Download `GET /api/v1/crates/{crate_name}/{version}/download`.
//! - [x] Yank `DELETE /api/v1/crates/{crate_name}/{version}/yank`.
//! - [x] Unyank `PUT /api/v1/crates/{crate_name}/{version}/unyank`.
//! - [ ] Owners List `GET /api/v1/crates/{crate_name}/owners`.
//! - [ ] Owners Add `PUT /api/v1/crates/{crate_name}/owners`.
//! - [ ] Owners Remove `DELETE /api/v1/crates/{crate_name}/owners`.
//! - [ ] Search `GET /api/v1/crates` query params: `q` (search terms), `per_page`
//!   (result limit - default 10, max 100).
//! - [x] Login `/me` (no details given re: method; cargo uses this for `cargo login`).

use crate::errors::ApiResult;
use crate::package_index::{Dependency, PackageIndex, PackageVersion};
use crate::Settings;
use actix_files as fs;
use actix_web::{delete, get, put, web, HttpRequest, HttpResponse, Responder};
use anyhow::Context;
use byteorder::{LittleEndian, ReadBytesExt};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Mutex;

pub type ApiResponse = ApiResult<HttpResponse>;

#[derive(Deserialize)]
pub struct Crate {
    crate_name: String,
    version: String,
}

/// Data supplied by `cargo` during the publishing of a crate.
///
/// The actual json payload has extra fields (which we're currently dropping)
/// but in the future we might want to record the data instead.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PartialPackageVersion {
    name: String,
    vers: String,
    deps: Vec<Dependency>,
    features: HashMap<String, Vec<String>>,
    links: Option<String>,
}

#[put("/new")]
pub async fn publish(
    mut payload: web::Bytes,
    package_index: web::Data<Mutex<PackageIndex>>,
    settings: web::Data<Settings>,
) -> ApiResponse {
    log::trace!("total len: {}", payload.len());

    let metadata_len = { payload.split_to(4).as_ref().read_u32::<LittleEndian>()? } as usize;
    log::trace!("metadata len: {}", metadata_len);

    let metadata: PartialPackageVersion =
        serde_json::from_slice(payload.split_to(metadata_len).as_ref())?;

    let crate_file_len = { payload.split_to(4).as_ref().read_u32::<LittleEndian>()? } as usize;
    log::trace!("crate file len: {}", crate_file_len);

    let crate_file_bytes = payload.split_to(crate_file_len);
    let cksum = format!("{:x}", Sha256::digest(crate_file_bytes.as_ref()));

    let pkg_version = PackageVersion {
        name: metadata.name,
        vers: metadata.vers,
        deps: metadata.deps,
        cksum,
        features: metadata.features,
        yanked: false,
        links: metadata.links,
    };

    let package_index = package_index.lock().unwrap();
    package_index.publish(&pkg_version)?;

    crate::storage::store_crate_file(
        &settings.crate_dir,
        &pkg_version.name,
        &pkg_version.vers,
        crate_file_bytes.as_ref(),
    )
    .context("Failed to store crate file.")?;
    Ok(HttpResponse::Ok().json(json!({
        // Optional object of warnings to display to the user.
        "warnings": {
            // Array of strings of categories that are invalid and ignored.
            "invalid_categories": [],
            // Array of strings of badge names that are invalid and ignored.
            "invalid_badges": [],
            // Array of strings of arbitrary warnings to display to the user.
            "other": []
        }
    })))
}

#[delete("/{crate_name}/{version}/yank")]
pub async fn yank(
    path: web::Path<Crate>,
    package_index: web::Data<Mutex<PackageIndex>>,
) -> ApiResponse {
    let package_index = package_index.lock().unwrap();
    package_index.set_yanked(&path.crate_name, &path.version, true)?;
    Ok(HttpResponse::Ok().json(json!({ "ok": true })))
}

#[put("/{crate_name}/{version}/unyank")]
pub async fn unyank(
    path: web::Path<Crate>,
    package_index: web::Data<Mutex<PackageIndex>>,
) -> ApiResponse {
    let index = package_index.lock().unwrap();
    index.set_yanked(&path.crate_name, &path.version, false)?;
    Ok(HttpResponse::Ok().json(json!({ "ok": true })))
}

#[get("/me")]
pub async fn login(_req: HttpRequest) -> impl Responder {
    HttpResponse::Ok().content_type("text/html").body(
        r#"
    <!doctype html>
    <html>
    <head/>
    <body>
        <dl>
            <dt>Your token is:</dt>
            <dd>
                <pre>0000</pre>
            </dd>
        </dl>
    </body>
    </html
    "#,
    )
}

#[get("/{crate_name}/{version}/download")]
pub async fn download(
    path: web::Path<Crate>,
    settings: web::Data<Settings>,
) -> ApiResult<fs::NamedFile> {
    let crate_file =
        crate::storage::get_crate_file_path(&settings.crate_dir, &path.crate_name, &path.version)?;
    log::debug!("serving `{}`", crate_file.display());
    Ok(fs::NamedFile::open(crate_file)?)
}

#[cfg(test)]
mod tests {
    use crate::package_index::{Config, PackageIndex};
    use crate::Settings;
    use actix_web::http::StatusCode;
    use actix_web::{test, web, App};
    use std::path::Path;
    use std::sync::Mutex;
    use tempdir::TempDir;

    /// This is the request body sent to the publish endpoint from an empty bin crate.
    const MY_CRATE_0_1_0: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/test_data/publish-my-crate-body"
    ));

    fn get_test_package_index(data_dir: &Path) -> web::Data<Mutex<PackageIndex>> {
        let config = Config {
            api: String::new(),
            dl: String::new(),
        };
        web::Data::new(Mutex::new(PackageIndex::init(data_dir, &config).unwrap()))
    }

    fn get_test_settings(data_dir: &Path) -> web::Data<Settings> {
        let settings = Settings {
            crate_dir: data_dir.join("crates").to_path_buf(),
            index_dir: data_dir.join("index").to_path_buf(),
        };
        web::Data::new(settings)
    }

    #[actix_rt::test]
    async fn test_login() {
        let data_root = TempDir::new("estuary_test").unwrap();
        let settings = get_test_settings(&data_root.path());
        let package_index = get_test_package_index(&settings.index_dir);

        let mut app = test::init_service(
            App::new()
                .app_data(package_index.clone())
                .app_data(settings.clone())
                .configure(crate::handlers::configure_routes),
        )
        .await;

        let req = test::TestRequest::get().uri("/me").to_request();
        let resp = test::call_service(&mut app, req).await;
        assert_eq!(StatusCode::OK, resp.status());
    }

    #[actix_rt::test]
    async fn test_publish() {
        let data_root = TempDir::new("estuary_test").unwrap();
        let settings = get_test_settings(&data_root.path());
        let package_index = get_test_package_index(&settings.index_dir);

        let mut app = test::init_service(
            App::new()
                .app_data(settings.clone())
                .app_data(package_index.clone())
                .configure(crate::handlers::configure_routes),
        )
        .await;

        let req = test::TestRequest::put()
            .uri("/api/v1/crates/new")
            .set_payload(MY_CRATE_0_1_0)
            .to_request();

        let resp: serde_json::Value = test::read_response_json(&mut app, req).await;
        assert!(!resp.as_object().unwrap().contains_key("errors"));
    }

    #[actix_rt::test]
    async fn test_publish_twice_is_error() {
        let data_root = TempDir::new("estuary_test").unwrap();
        let settings = get_test_settings(&data_root.path());
        let package_index = get_test_package_index(&settings.index_dir);

        let mut app = test::init_service(
            App::new()
                .app_data(settings.clone())
                .app_data(package_index.clone())
                .configure(crate::handlers::configure_routes),
        )
        .await;

        // First publish
        let req = test::TestRequest::put()
            .uri("/api/v1/crates/new")
            .set_payload(MY_CRATE_0_1_0)
            .to_request();

        let resp: serde_json::Value = test::read_response_json(&mut app, req).await;
        // No errors the first time
        assert!(!resp.as_object().unwrap().contains_key("errors"));

        // Second publish
        let req = test::TestRequest::put()
            .uri("/api/v1/crates/new")
            .set_payload(MY_CRATE_0_1_0)
            .to_request();

        let resp: serde_json::Value = test::read_response_json(&mut app, req).await;
        // There should be errors in this case...
        assert!(resp.as_object().unwrap().contains_key("errors"));
    }

    #[actix_rt::test]
    async fn test_yank() {
        let data_root = TempDir::new("estuary_test").unwrap();
        let settings = get_test_settings(&data_root.path());
        let package_index = get_test_package_index(&settings.index_dir);

        let mut app = test::init_service(
            App::new()
                .app_data(settings.clone())
                .app_data(package_index.clone())
                .configure(crate::handlers::configure_routes),
        )
        .await;

        // Publish (so we can yank)
        let req = test::TestRequest::put()
            .uri("/api/v1/crates/new")
            .set_payload(MY_CRATE_0_1_0)
            .to_request();

        let _: serde_json::Value = test::read_response_json(&mut app, req).await;

        let req = test::TestRequest::delete()
            .uri("/api/v1/crates/my-crate/0.1.0/yank")
            .to_request();

        let resp: serde_json::Value = test::read_response_json(&mut app, req).await;
        assert!(resp["ok"].as_bool().unwrap());
    }

    #[actix_rt::test]
    async fn test_unyank() {
        assert_eq!(1101, MY_CRATE_0_1_0.len());
        let data_root = TempDir::new("estuary_test").unwrap();
        let settings = get_test_settings(&data_root.path());
        let package_index = get_test_package_index(&settings.index_dir);

        let mut app = test::init_service(
            App::new()
                .app_data(settings.clone())
                .app_data(package_index.clone())
                .configure(crate::handlers::configure_routes),
        )
        .await;

        // Publish (so we can yank)
        let req = test::TestRequest::put()
            .uri("/api/v1/crates/new")
            .set_payload(MY_CRATE_0_1_0)
            .to_request();

        let _: serde_json::Value = test::read_response_json(&mut app, req).await;

        let req = test::TestRequest::delete()
            .uri("/api/v1/crates/my-crate/0.1.0/yank")
            .to_request();

        let _: serde_json::Value = test::read_response_json(&mut app, req).await;

        let req = test::TestRequest::put()
            .uri("/api/v1/crates/my-crate/0.1.0/unyank")
            .to_request();

        let resp: serde_json::Value = test::read_response_json(&mut app, req).await;
        assert!(resp["ok"].as_bool().unwrap());
    }
}
