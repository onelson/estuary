//! These endpoints relate to the core packaging features.
//!
//! Publish, yank, unyank, and download are the bare essentials needed for
//! adding new crates to the registry and using the registry to install crates.
//!
//! For now, the endpoints related to "owners" are on the back burner.
//! For the small-scale use case estuary targets, we may not need them at all.
//!
//! Still TODO:
//! - Owners List `GET /api/v1/crates/{crate_name}/owners`.
//! - Owners Add `PUT /api/v1/crates/{crate_name}/owners`.
//! - Owners Remove `DELETE /api/v1/crates/{crate_name}/owners`.
//!
use crate::errors::{ApiError, EstuaryError};
use crate::package_index::{PackageIndex, PackageVersion};
use crate::Settings;
use actix_files as fs;
use actix_web::{delete, get, put, web, HttpResponse};
use byteorder::{LittleEndian, ReadBytesExt};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::sync::Mutex;

pub type ApiResponse = Result<HttpResponse, ApiError>;

#[derive(Deserialize)]
pub(crate) struct Crate {
    crate_name: String,
    version: semver::Version,
}

#[put("/new")]
pub(crate) async fn publish(
    mut payload: web::Bytes,
    package_index: web::Data<Mutex<PackageIndex>>,
    settings: web::Data<Settings>,
) -> ApiResponse {
    log::trace!("total len: {}", payload.len());

    let metadata_len = {
        payload
            .split_to(4)
            .as_ref()
            .read_u32::<LittleEndian>()
            .map_err(EstuaryError::from)?
    } as usize;
    log::trace!("metadata len: {}", metadata_len);

    let new_crate_metadata: crate::database::NewCrate =
        serde_json::from_slice(payload.split_to(metadata_len).as_ref())
            .map_err(EstuaryError::from)?;

    let crate_file_len = {
        payload
            .split_to(4)
            .as_ref()
            .read_u32::<LittleEndian>()
            .map_err(EstuaryError::from)?
    } as usize;
    log::trace!("crate file len: {}", crate_file_len);

    let crate_file_bytes = payload.split_to(crate_file_len);
    let cksum = format!("{:x}", Sha256::digest(crate_file_bytes.as_ref()));
    let pkg_version = PackageVersion::from_new_crate(&new_crate_metadata, &cksum)?;

    let mut conn = settings.get_db()?;
    crate::database::publish(&mut conn, &new_crate_metadata)?;
    let package_index = package_index.lock().unwrap();
    package_index.publish(&pkg_version)?;

    crate::storage::store_crate_file(
        &settings.crate_dir,
        &pkg_version.name,
        &pkg_version.vers,
        crate_file_bytes.as_ref(),
    )?;
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
pub(crate) async fn yank(
    path: web::Path<Crate>,
    package_index: web::Data<Mutex<PackageIndex>>,
) -> ApiResponse {
    // FIXME: update database and index (both)
    let package_index = package_index.lock().unwrap();
    package_index.set_yanked(&path.crate_name, &path.version, true)?;
    Ok(HttpResponse::Ok().json(json!({ "ok": true })))
}

#[put("/{crate_name}/{version}/unyank")]
pub(crate) async fn unyank(
    path: web::Path<Crate>,
    package_index: web::Data<Mutex<PackageIndex>>,
) -> ApiResponse {
    // FIXME: update database and index (both)
    let index = package_index.lock().unwrap();
    index.set_yanked(&path.crate_name, &path.version, false)?;
    Ok(HttpResponse::Ok().json(json!({ "ok": true })))
}

#[get("/{crate_name}/{version}/download")]
pub(crate) async fn download(
    path: web::Path<Crate>,
    settings: web::Data<Settings>,
) -> actix_web::Result<fs::NamedFile> {
    let crate_file =
        crate::storage::get_crate_file_path(&settings.crate_dir, &path.crate_name, &path.version);
    log::debug!("serving `{}`", crate_file.display());
    Ok(fs::NamedFile::open(crate_file)?)
}

/// Query string params for the search endpoint.
///
/// At time of writing, the spec mentions a per page parameter to limit the
/// number of results, but doesn't talk about how to express the offset or
/// page number.
///
/// <https://doc.rust-lang.org/nightly/cargo/reference/registries.html#search>
#[derive(Deserialize, Debug)]
pub(crate) struct SearchQuery {
    /// The search terms to match on.
    q: String,
    /// default=10, max=100.
    ///
    /// Note that `cargo` itself will clamp the value at 100 if the `--limit`
    /// flag is set to a higher number.
    per_page: usize,
}

#[derive(Serialize, Debug)]
pub(crate) struct SearchResult {
    name: String,
    max_version: semver::Version,
    description: String,
}

#[get("")]
pub(crate) async fn search(
    query: web::Query<SearchQuery>,
    index: web::Data<Mutex<PackageIndex>>,
) -> ApiResponse {
    // FIXME: look at database instead of index on disk

    let index = index.lock().unwrap();
    let names = index.list_crates()?;
    let terms: Vec<&str> = query.q.split(&['-', '_', ' ', '\t'][..]).collect();
    let mut matches: Vec<(&str, usize)> = names
        .iter()
        .filter_map(|name| {
            let mut score = terms.iter().filter(|&&term| name.contains(term)).count();
            if name == &query.q {
                score += 100; // idk, if the search is an exact match, boost it.
            }
            if score > 0 {
                Some((name.as_str(), score))
            } else {
                None
            }
        })
        .collect();

    let total_match_count = matches.len();
    matches.sort_by_key(|(_, score)| 0_isize - *score as isize);

    let crates: Result<Vec<SearchResult>, _> = matches
        .into_iter()
        .map(|(name, _)| {
            index.get_package_versions(name).map(|pkgs| {
                pkgs.into_iter()
                    .filter(|pkg| !pkg.yanked)
                    .max_by(|a, b| a.vers.cmp(&b.vers))
                    .map(|pkg| SearchResult {
                        name: pkg.name,
                        max_version: pkg.vers,
                        // FIXME: need a db to hold on to this info
                        description: String::new(),
                    })
            })
        })
        .filter_map(|res: Result<Option<_>, _>| match res {
            // Errors should be propagated so we can deal with them in the
            // handler body.
            Err(e) => Some(Err(e)),
            Ok(Some(pkg)) => Some(Ok(pkg)),
            // filter out crates that don't have any unyanked versions.
            Ok(None) => None,
        })
        .take(query.per_page)
        .collect();

    Ok(HttpResponse::Ok().json(json!({
    "crates": crates?,
    "meta": {
        "total": total_match_count
    }
    })))
}

#[cfg(test)]
mod tests {
    use crate::test_helpers;
    use crate::test_helpers::MY_CRATE_0_1_0;
    use actix_web::http::StatusCode;
    use actix_web::{test, App};

    #[actix_rt::test]
    async fn test_publish() {
        let data_root = test_helpers::get_data_root();
        let settings = test_helpers::get_test_settings(&data_root.path());
        let package_index = test_helpers::get_test_package_index(&settings.index_dir);

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
        let data_root = test_helpers::get_data_root();
        let settings = test_helpers::get_test_settings(&data_root.path());
        let package_index = test_helpers::get_test_package_index(&settings.index_dir);

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
        let data_root = test_helpers::get_data_root();
        let settings = test_helpers::get_test_settings(&data_root.path());
        let package_index = test_helpers::get_test_package_index(&settings.index_dir);

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
        let data_root = test_helpers::get_data_root();
        let settings = test_helpers::get_test_settings(&data_root.path());
        let package_index = test_helpers::get_test_package_index(&settings.index_dir);

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

    #[actix_rt::test]
    async fn test_download_existing_crate_is_ok() {
        let data_root = test_helpers::get_data_root();
        let settings = test_helpers::get_test_settings(&data_root.path());
        let package_index = test_helpers::get_test_package_index(&settings.index_dir);

        let mut app = test::init_service(
            App::new()
                .app_data(settings.clone())
                .app_data(package_index.clone())
                .configure(crate::handlers::configure_routes),
        )
        .await;

        // Publish (so we can download)
        let req = test::TestRequest::put()
            .uri("/api/v1/crates/new")
            .set_payload(MY_CRATE_0_1_0)
            .to_request();

        let _: serde_json::Value = test::read_response_json(&mut app, req).await;

        let req = test::TestRequest::get()
            .uri("/api/v1/crates/my-crate/0.1.0/download")
            .to_request();

        let resp = test::call_service(&mut app, req).await;
        assert_eq!(StatusCode::OK, resp.status());
    }

    #[actix_rt::test]
    async fn test_download_nonexistent_crate_is_not_found() {
        let data_root = test_helpers::get_data_root();
        let settings = test_helpers::get_test_settings(&data_root.path());
        let package_index = test_helpers::get_test_package_index(&settings.index_dir);

        let mut app = test::init_service(
            App::new()
                .app_data(settings.clone())
                .app_data(package_index.clone())
                .configure(crate::handlers::configure_routes),
        )
        .await;

        // No crates have been published
        let req = test::TestRequest::get()
            .uri("/api/v1/crates/my-crate/0.1.0/download")
            .to_request();

        let resp = test::call_service(&mut app, req).await;
        assert_eq!(StatusCode::NOT_FOUND, resp.status());
    }
}
