use actix_web::web;

pub mod auth;

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/auth")
            .service(auth::register)
            .service(auth::login),
    );
}
