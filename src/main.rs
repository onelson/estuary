use crate::errors::EstuaryError;
use actix_web::{middleware, web, App, HttpServer};
use package_index::{Config, PackageIndex};
use std::path::PathBuf;
use std::sync::Mutex;

mod cli;
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

#[cfg(not(tarpaulin_include))]
#[actix_web::main]
async fn main() -> Result<(), EstuaryError> {
    #[cfg(feature = "dotenv")]
    dotenv::dotenv().ok();

    env_logger::init();

    let args = cli::parse_args();

    let bind_addr = format!("{}:{}", args.http_host, args.http_port);
    let config = Config {
        dl: args.download_url(),
        api: args.base_url().to_string(),
    };
    let settings = Settings {
        crate_dir: args.crate_dir,
        index_dir: args.index_dir,
        git_binary: args.git_bin,
    };

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
