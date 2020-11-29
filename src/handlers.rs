use actix_web::web;

pub mod registry;

pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(registry::login).service(
        web::scope("/api/v1/crates")
            .service(registry::publish)
            .service(registry::yank)
            .service(registry::unyank)
            .service(registry::download),
    );
}
