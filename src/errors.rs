use actix_web::dev::HttpResponseBuilder;
use actix_web::error::ResponseError;
use actix_web::http::StatusCode;
use actix_web::HttpResponse;
use anyhow::Error;
use serde_json::json;
use std::fmt::Display;

pub type ApiResult<T> = std::result::Result<T, ApiError>;

#[derive(Debug)]
pub struct ApiError(Error);

impl Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Error> for ApiError {
    fn from(other: Error) -> Self {
        Self(other)
    }
}

impl From<std::io::Error> for ApiError {
    fn from(other: std::io::Error) -> Self {
        Self(other.into())
    }
}

impl From<serde_json::Error> for ApiError {
    fn from(other: serde_json::Error) -> Self {
        Self(other.into())
    }
}

/// Errors are converted to a 200 OK response with a json body (eugh).
/// Cargo will present "detail" keys to the user.
impl ResponseError for ApiError {
    fn status_code(&self) -> StatusCode {
        StatusCode::OK
    }

    fn error_response(&self) -> HttpResponse {
        HttpResponseBuilder::new(self.status_code())
            .json(json!({"errors": [{ "detail": format!("{}", self.0) }]}))
    }
}

pub type GitResult<T> = std::result::Result<T, GitError>;

#[derive(Debug)]
pub struct GitError(Error);

impl Display for GitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Error> for GitError {
    fn from(other: Error) -> Self {
        Self(other)
    }
}

impl From<actix_web::error::BlockingError<Error>> for GitError {
    fn from(other: actix_web::error::BlockingError<Error>) -> Self {
        Self(other.into())
    }
}

impl From<std::io::Error> for GitError {
    fn from(other: std::io::Error) -> Self {
        Self(other.into())
    }
}

impl ResponseError for GitError {
    fn status_code(&self) -> StatusCode {
        StatusCode::INTERNAL_SERVER_ERROR
    }

    fn error_response(&self) -> HttpResponse {
        HttpResponseBuilder::new(self.status_code()).body(format!("{}", self.0))
    }
}
