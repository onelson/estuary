use crate::cli::Command;
use crate::errors::EstuaryError;
use actix_web::{middleware, web, App, HttpServer};
use package_index::{Config, PackageIndex};
use std::path::PathBuf;
use std::sync::Mutex;

mod cli;
mod database;
mod errors;
mod handlers;
mod package_index;
mod storage;

type Result<T> = std::result::Result<T, EstuaryError>;

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
    fn get_db(&self) -> Result<rusqlite::Connection> {
        Ok(rusqlite::Connection::open(
            self.index_dir.join("estuary.sqlite"),
        )?)
    }
}

#[cfg(not(tarpaulin_include))]
#[actix_web::main]
async fn main() -> Result<()> {
    #[cfg(feature = "dotenv")]
    dotenv::dotenv().ok();

    env_logger::init();

    let args = cli::parse_args();

    match args.cmd {
        Command::Run(run_opt) => {
            let config = Config {
                dl: run_opt.download_url(),
                api: run_opt.base_url().to_string(),
            };
            let settings = Settings {
                crate_dir: run_opt.crate_dir,
                index_dir: args.index_dir,
                git_binary: args.git_bin,
            };
            return Ok(run_server(&run_opt.http_host, run_opt.http_port, config, settings).await?);
        }
        Command::BackfillDb => {
            let settings = Settings {
                // FIXME: we need a Settings to get a db conn, but Settings expects to know about more info than we want to know here.
                crate_dir: Default::default(),
                index_dir: args.index_dir,
                git_binary: args.git_bin,
            };
            let package_index = PackageIndex::new(&settings.index_dir)?;

            let mut conn = settings.get_db()?;
            database::init(&conn)?;
            database::backfill_db(&mut conn, &package_index)
        }
    }
}

async fn run_server(
    host: &str,
    port: u16,
    config: Config,
    settings: Settings,
) -> crate::Result<()> {
    let bind_addr = format!("{}:{}", host, port);

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

    let conn = settings.get_db()?;
    database::init(&conn)?;

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
