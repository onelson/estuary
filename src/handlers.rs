use actix_web::web;

pub mod git;
pub mod registry;

pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(registry::login)
        .service(
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
        );
}
