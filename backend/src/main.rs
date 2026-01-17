use actix_cors::Cors;
use actix_web::{App, HttpResponse, HttpServer, Responder, get, web};
use env_logger;

mod api;
mod db;
mod integrations;
mod models;
mod schema;
mod services;

#[get("/")]
async fn hello() -> impl Responder {
    HttpResponse::Ok().body("PVPC Cheap Backend (Actix) Running!")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    // DB Pool initialization
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = db::init_pool(&database_url);

    // Run migrations? (Optional, good for dev)
    // For now we assume docker-compose handles it or we run diesel CLI.

    log::info!("Starting HTTP server at http://0.0.0.0:8080");

    HttpServer::new(move || {
        App::new()
            .wrap(Cors::permissive())
            .app_data(web::Data::new(pool.clone()))
            .service(hello)
            .configure(api::config)
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}
