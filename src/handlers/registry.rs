use crate::errors::ApiResult;
use crate::package_index::{Dependency, PackageIndex, PackageVersion};
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
) -> ApiResponse {
    let crate_dir = std::env::var("ESTUARY_CRATE_DIR").expect("ESTUARY_CRATE_DIR");
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
        crate_dir,
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

#[get("/{crate_name}/owners")]
pub async fn owners_list(_req: HttpRequest) -> impl Responder {
    "owners list" // FIXME
}

#[put("/{crate_name}/owners")]
pub async fn owners_add(_req: HttpRequest) -> impl Responder {
    "owners add" // FIXME
}

#[delete("/{crate_name}/owners")]
pub async fn owners_remove(_req: HttpRequest) -> impl Responder {
    "owners remove" // FIXME
}

#[get("")]
pub async fn search(_req: HttpRequest) -> impl Responder {
    "crate search" // FIXME
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
pub async fn download(path: web::Path<Crate>) -> ApiResult<fs::NamedFile> {
    let crate_dir = std::env::var("ESTUARY_CRATE_DIR").expect("ESTUARY_CRATE_DIR");
    let crate_file =
        crate::storage::get_crate_file_path(crate_dir, &path.crate_name, &path.version)?;
    log::debug!("serving `{}`", crate_file.display());
    Ok(fs::NamedFile::open(crate_file)?)
}
