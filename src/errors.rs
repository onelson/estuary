#![cfg(not(tarpaulin_include))]
use actix_web::dev::HttpResponseBuilder;
use actix_web::error::{BlockingError, ResponseError};
use actix_web::http::StatusCode;
use actix_web::HttpResponse;
use serde_json::json;
use std::fmt::{Debug, Display};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error(transparent)]
    Estuary(#[from] EstuaryError),
}

/// For the Api Errors, cargo wants them converted to a 200 OK response with a
/// json body (eugh).
/// Cargo will present "detail" keys to the user.
impl ResponseError for ApiError {
    fn status_code(&self) -> StatusCode {
        StatusCode::OK
    }

    fn error_response(&self) -> HttpResponse {
        HttpResponseBuilder::new(self.status_code())
            .json(json!({"errors": [{ "detail": self.to_string() }]}))
    }
}

#[derive(Debug, Error)]
pub enum EstuaryError {
    #[error("Git error: `{0}`")]
    Git2(#[from] git2::Error),
    #[error("Publish failed: `{0}`")]
    Publish(String),
    #[error("Invalid package name: `{0}`")]
    InvalidPackageName(String),
    #[error("Glob failed: `{0}`")]
    Glob(#[from] glob::GlobError),
    #[error("Glob pattern failed: `{0}`")]
    GlobPattern(#[from] glob::PatternError),
    #[error("Invalid dependency kind: `{0}`")]
    DependencyKind(String),
    #[error("JSON parse failed: `{0}`")]
    JSON(#[from] serde_json::Error),
    #[error("IO error: `{0}`")]
    IO(#[from] std::io::Error),
    #[error("Blocking task canceled")]
    BlockingTaskCanceled,
    #[error("Not Found")]
    NotFound,
    #[error("Invalid Version: `{0}`")]
    InvalidVersion(#[from] semver::SemVerError),
    #[error("DB error: `{0}`")]
    DbError(#[from] rusqlite::Error),
}

impl<T> From<BlockingError<T>> for EstuaryError
where
    T: Into<EstuaryError> + Display + Debug,
{
    fn from(e: BlockingError<T>) -> Self {
        match e {
            BlockingError::Canceled => EstuaryError::BlockingTaskCanceled,
            BlockingError::Error(err) => err.into(),
        }
    }
}

impl ResponseError for EstuaryError {
    fn status_code(&self) -> StatusCode {
        match self {
            EstuaryError::NotFound => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse {
        HttpResponseBuilder::new(self.status_code()).body(self.to_string())
    }
}
