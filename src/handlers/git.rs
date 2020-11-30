//! Cargo reads index information using git.
//!
//! This means we need a read-only way to advertise the index data that supports
//! git's "smart" HTTP transport.
//!
//! The endpoints here aim to support whatever is necessary for "git fetch" to
//! work so cargo can do what it needs.

use crate::errors::GitResult;
use crate::Settings;
use actix_web::{get, post, web, HttpResponse};
use serde::Deserialize;
use std::io::Write;
use std::process::{Command, Stdio};

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
) -> GitResult<HttpResponse> {
    let service_name = query.service.as_service_name();
    let output = Command::new("git")
        .args(&[
            service_name,
            "--stateless-rpc",
            "--advertise-refs",
            &settings.index_dir.display().to_string(),
        ])
        .output()?;

    log::trace!("git says: {:?}", &output);

    let mut body = vec![];

    write!(
        body,
        "{}",
        pkt_line(&format!("# service=git-{}\n", service_name))
    )?;

    write!(body, "0000")?;
    body.extend(output.stdout);

    Ok(HttpResponse::Ok()
        .content_type(format!("application/x-git-{}-advertisement", service_name))
        .body(body))
}

#[post("/git-upload-pack")]
pub async fn upload_pack(
    settings: web::Data<Settings>,
    payload: web::Bytes,
) -> GitResult<HttpResponse> {
    let service_name = Service::UploadPack.as_service_name();
    let mut cmd = Command::new("git")
        .args(&[
            service_name,
            "--stateless-rpc",
            &settings.index_dir.display().to_string(),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    cmd.stdin.as_mut().unwrap().write_all(&payload)?;
    let output = cmd.wait_with_output()?;

    if output.status.success() {
        Ok(HttpResponse::Ok()
            .content_type(format!("application/x-git-{}-result", service_name))
            .body(output.stdout))
    } else {
        log::error!(
            "git upload-pack with: `{}`",
            String::from_utf8_lossy(&output.stderr)
        );
        Ok(HttpResponse::InternalServerError().finish())
    }
}

#[cfg(test)]
mod tests {
    use crate::handlers::git::pkt_line;

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
}
