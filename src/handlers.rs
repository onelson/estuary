use actix_web::web;
pub mod frontend;
pub mod git;
pub mod registry;

pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/git/index")
            .service(git::get_info_refs)
            .service(git::upload_pack),
    )
    .service(
        web::scope("/api/v1/crates")
            .service(registry::publish)
            .service(registry::yank)
            .service(registry::unyank)
            .service(registry::download),
    )
    .service(frontend::styles)
    .service(frontend::login)
    .service(frontend::landing)
    .service(
        web::scope("/crates/{crate_name}")
            .route("/versions", web::get().to(frontend::version_list))
            .route("/{version}", web::get().to(frontend::crate_detail))
            .route("", web::get().to(frontend::crate_detail)),
    );
}
