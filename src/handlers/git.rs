//! Cargo reads index information using git.
//!
//! This means we need a read-only way to advertise the index data that supports
//! git's "smart" HTTP transport.
//!
//! The endpoints here aim to support whatever is necessary for "git fetch" to
//! work so cargo can do what it needs.

use crate::errors::EstuaryError;
use crate::Settings;
use actix_web::{get, post, web, HttpResponse};
use serde::Deserialize;
use std::io::Write;
use std::process::{Command, Stdio};

type Result<T> = std::result::Result<T, EstuaryError>;

/// Prefixes the string with 4 bytes representing the hex length of the string.
///
/// The lines git's response bodies use a packet protocol where the first 4
/// bytes are the hex value of the length of the line (including the hex prefix).
///
/// Lines that include newline characters should have a literal `\n` in the
/// string so it can be included in the length computed here.
fn pkt_line(s: &str) -> String {
    format!("{:04x}{}", s.len() + 4, s)
}

/// Git "services" offered by our transport.
#[derive(Deserialize)]
pub enum Service {
    #[serde(rename = "git-upload-pack")]
    UploadPack,
}

impl Service {
    pub fn as_service_name(&self) -> &str {
        match self {
            Self::UploadPack => "upload-pack",
        }
    }
}

#[derive(Deserialize)]
pub struct Query {
    service: Service,
}

#[get("/info/refs")]
pub async fn get_info_refs(
    settings: web::Data<Settings>,
    query: web::Query<Query>,
) -> Result<HttpResponse> {
    let service_name = query.service.as_service_name().to_string();
    let svc = service_name.clone();
    let output = web::block(move || {
        let service_name = svc;
        Command::new(&settings.git_binary)
            .args(&[
                &service_name,
                "--stateless-rpc",
                "--advertise-refs",
                &settings.index_dir.display().to_string(),
            ])
            .output()
    })
    .await?;

    log::trace!("git says: {:?}", &output);

    let mut body = vec![];

    write!(
        body,
        "{}",
        pkt_line(&format!("# service=git-{}\n", &service_name))
    )?;

    write!(body, "0000")?;
    body.extend(output.stdout);

    Ok(HttpResponse::Ok()
        .content_type(format!("application/x-git-{}-advertisement", &service_name))
        .body(body))
}

#[post("/git-upload-pack")]
pub async fn upload_pack(
    settings: web::Data<Settings>,
    payload: web::Bytes,
) -> Result<HttpResponse> {
    let service_name = Service::UploadPack.as_service_name();

    let output = web::block(move || {
        let mut cmd = Command::new(&settings.git_binary)
            .args(&[
                service_name,
                "--stateless-rpc",
                &settings.index_dir.display().to_string(),
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;

        cmd.stdin.as_mut().unwrap().write_all(&payload)?;
        cmd.wait_with_output()
    })
    .await?;

    if output.status.success() {
        Ok(HttpResponse::Ok()
            .content_type(format!("application/x-git-{}-result", service_name))
            .body(output.stdout))
    } else {
        log::error!("git upload-pack failed with: `{:?}`", &output);
        Ok(HttpResponse::InternalServerError().finish())
    }
}

#[cfg(test)]
mod tests {
    use crate::handlers::git::pkt_line;
    use crate::test_helpers;
    use actix_web::http::StatusCode;
    use actix_web::{test, App};

    #[test]
    fn test_pkt_line_from_example() {
        let input = "d049f6c27a2244e12041955e262a404c7faba355 refs/heads/master\n";
        let expected = "003fd049f6c27a2244e12041955e262a404c7faba355 refs/heads/master\n";
        assert_eq!(expected, pkt_line(input));
    }

    #[test]
    fn test_pkt_line_empty() {
        let input = "";
        let expected = "0004";
        assert_eq!(expected, pkt_line(input));
    }

    #[actix_rt::test]
    async fn test_get_info_refs_no_service_query() {
        let data_root = test_helpers::get_data_root();
        let settings = test_helpers::get_test_settings(&data_root.path());
        let package_index = test_helpers::get_test_package_index(&settings.index_dir);
        let mut app = test::init_service(
            App::new()
                .app_data(package_index.clone())
                .app_data(settings.clone())
                .configure(crate::handlers::configure_routes),
        )
        .await;
        let req = test::TestRequest::get()
            .uri("/git/index/info/refs")
            .to_request();
        let resp = test::call_service(&mut app, req).await;
        assert_eq!(StatusCode::BAD_REQUEST, resp.status());
    }

    #[actix_rt::test]
    async fn test_get_info_refs_invalid_service_query() {
        let data_root = test_helpers::get_data_root();
        let settings = test_helpers::get_test_settings(&data_root.path());
        let package_index = test_helpers::get_test_package_index(&settings.index_dir);
        let mut app = test::init_service(
            App::new()
                .app_data(package_index.clone())
                .app_data(settings.clone())
                .configure(crate::handlers::configure_routes),
        )
        .await;
        let req = test::TestRequest::get()
            .uri("/git/index/info/refs?service=something%20invalid")
            .to_request();
        let resp = test::call_service(&mut app, req).await;
        assert_eq!(StatusCode::BAD_REQUEST, resp.status());
    }

    #[actix_rt::test]
    async fn test_get_info_refs_valid_service_query() {
        let data_root = test_helpers::get_data_root();
        let settings = test_helpers::get_test_settings(&data_root.path());
        let package_index = test_helpers::get_test_package_index(&settings.index_dir);
        let mut app = test::init_service(
            App::new()
                .app_data(package_index.clone())
                .app_data(settings.clone())
                .configure(crate::handlers::configure_routes),
        )
        .await;
        let req = test::TestRequest::get()
            .uri("/git/index/info/refs?service=git-upload-pack")
            .to_request();
        let resp = test::call_service(&mut app, req).await;
        assert_eq!(StatusCode::OK, resp.status());
    }

    #[actix_rt::test]
    async fn test_upload_pack_no_body() {
        let data_root = test_helpers::get_data_root();
        let settings = test_helpers::get_test_settings(&data_root.path());
        let package_index = test_helpers::get_test_package_index(&settings.index_dir);

        let mut app = test::init_service(
            App::new()
                .app_data(package_index.clone())
                .app_data(settings.clone())
                .configure(crate::handlers::configure_routes),
        )
        .await;
        let req = test::TestRequest::post()
            .uri("/git/index/git-upload-pack")
            .header("content-type", "application/x-git-upload-pack-request")
            .header("accept", "application/x-git-upload-pack-result")
            .to_request();
        let resp = test::call_service(&mut app, req).await;
        assert_eq!(StatusCode::INTERNAL_SERVER_ERROR, resp.status());
    }

    #[actix_rt::test]
    async fn test_upload_pack_initial_fetch() {
        let data_root = test_helpers::get_data_root();
        let settings = test_helpers::get_test_settings(&data_root.path());
        let package_index = test_helpers::get_test_package_index(&settings.index_dir);
        let mut app = test::init_service(
            App::new()
                .app_data(package_index.clone())
                .app_data(settings.clone())
                .configure(crate::handlers::configure_routes),
        )
        .await;
        let req = test::TestRequest::post()
            .uri("/git/index/git-upload-pack")
            .header("content-type", "application/x-git-upload-pack-request")
            .header("accept", "application/x-git-upload-pack-result")
            .set_payload("0000") // empty fetch, "don't care what you have"
            .to_request();
        let resp = test::call_service(&mut app, req).await;
        assert_eq!(StatusCode::OK, resp.status());
    }
}
