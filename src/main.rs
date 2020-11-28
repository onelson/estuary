use actix_web::{middleware, web, App, HttpServer};
use handlers::registry;
use package_index::{Config, PackageIndex};

use std::env;
use std::path::PathBuf;
use std::sync::Mutex;

mod errors;
mod handlers;
mod package_index;
mod storage;

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();
    env_logger::init();

    let index_dir: PathBuf = env::var("ESTUARY_INDEX_DIR")
        .expect("ESTUARY_INDEX_DIR")
        .into();
    let crate_dir = env::var("ESTUARY_CRATE_DIR").expect("ESTUARY_CRATE_DIR");

    let host = env::var("HOST").unwrap_or_else(|_| String::from("0.0.0.0"));
    let port: u16 = env::var("PORT")
        .unwrap_or_else(|_| String::from("7878"))
        .parse()?;

    let bind_addr = format!("{}:{}", host, port);
    let config = Config::from_env()?;

    log::info!("Server starting on `{}`", bind_addr);
    log::info!("\tIndex Dir: `{}`", index_dir.display());
    log::info!("\tCrate Dir: `{}`", crate_dir);
    log::info!("\tPackage Index Config: `{:?}`", config);

    let package_index = web::Data::new(Mutex::new(PackageIndex::init(&index_dir, &config)?));

    Ok(HttpServer::new(move || {
        App::new()
            .wrap(middleware::Logger::default())
            .app_data(package_index.clone())
            .service(registry::login)
            .service(
                web::scope("/api/v1/crates")
                    .service(registry::publish)
                    .service(registry::yank)
                    .service(registry::unyank)
                    .service(registry::owners_add)
                    .service(registry::owners_remove)
                    .service(registry::owners_list)
                    .service(registry::search)
                    .service(registry::download),
            )
    })
    .bind(bind_addr)?
    .run()
    .await?)
}
