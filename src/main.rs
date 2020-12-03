use actix_web::{middleware, web, App, HttpServer};
use anyhow::{anyhow, Context, Result};
use package_index::{Config, PackageIndex};
use std::env;
use std::path::PathBuf;
use std::sync::Mutex;

mod errors;
mod handlers;
mod package_index;
mod storage;

/// Common configuration details to share with handlers.
#[derive(Clone, Debug)]
pub struct Settings {
    /// Root path for storing `.crate` files when they are published.
    pub crate_dir: PathBuf,
    /// Location for the git repo that tracks changes to the package index.
    ///
    /// Note that this should be the path to the working tree, not the `.git`
    /// directory inside it.
    pub index_dir: PathBuf,
    /// Optionally specify a path to `git`.
    ///
    /// Defaults to just "git", expecting it to be in your `PATH`.
    pub git_binary: PathBuf,
}

impl Settings {
    pub fn from_env() -> Result<Self> {
        let index_dir: PathBuf = env::var("ESTUARY_INDEX_DIR")
            .context("ESTUARY_INDEX_DIR")?
            .into();
        let crate_dir: PathBuf = env::var("ESTUARY_CRATE_DIR")
            .context("ESTUARY_CRATE_DIR")?
            .into();

        let git_binary = if let Ok(s) = env::var("ESTUARY_GIT_BIN") {
            match PathBuf::from(s)
                .canonicalize()
                .context("Invalid value ESTUARY_GIT_BIN")?
            {
                pb if pb.is_file() => pb,
                _ => return Err(anyhow!("Invalid value ESTUARY_GIT_BIN")),
            }
        } else {
            PathBuf::from("git")
        };

        Ok(Self {
            crate_dir,
            index_dir,
            git_binary,
        })
    }
}

#[cfg(not(tarpaulin_include))]
#[actix_web::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();
    env_logger::init();

    let host = env::var("ESTUARY_HOST").unwrap_or_else(|_| String::from("0.0.0.0"));
    let port: u16 = env::var("ESTUARY_PORT")
        .unwrap_or_else(|_| String::from("7878"))
        .parse()?;

    let bind_addr = format!("{}:{}", host, port);
    let config = Config::from_env()?;
    let settings = Settings::from_env()?;

    std::fs::create_dir_all(&settings.index_dir)?;
    std::fs::create_dir_all(&settings.crate_dir)?;

    log::info!("Server starting on `{}`", bind_addr);
    log::info!("\tIndex Dir: `{}`", settings.index_dir.display());
    log::info!("\tCrate Dir: `{}`", settings.crate_dir.display());
    log::info!("\tPackage Index Config: `{:?}`", config);

    let package_index = web::Data::new(Mutex::new(PackageIndex::init(
        &settings.index_dir,
        &config,
    )?));

    Ok(HttpServer::new(move || {
        App::new()
            .wrap(middleware::Logger::default())
            .app_data(package_index.clone())
            .data(settings.clone())
            .configure(handlers::configure_routes)
    })
    .bind(bind_addr)?
    .run()
    .await?)
}

#[cfg(test)]
mod test_helpers;
