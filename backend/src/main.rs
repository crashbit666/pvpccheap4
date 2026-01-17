use actix_cors::Cors;
use actix_web::{get, web, App, HttpResponse, HttpServer, Responder};

mod api;
mod db;
mod integrations;
mod models;
mod schema;
mod services;

use integrations::ProviderRegistry;

#[get("/")]
async fn health_check() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "ok",
        "service": "PVPC Cheap Backend",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

#[get("/api/providers")]
async fn list_providers(registry: web::Data<ProviderRegistry>) -> impl Responder {
    let providers: Vec<_> = registry
        .available_providers()
        .iter()
        .map(|name| {
            let provider = registry.get(name).unwrap();
            serde_json::json!({
                "name": provider.provider_name(),
                "display_name": provider.display_name(),
                "capabilities": provider.get_capabilities()
            })
        })
        .collect();

    HttpResponse::Ok().json(providers)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    // DB Pool initialization
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = db::init_pool(&database_url);

    // Provider registry
    let provider_registry = ProviderRegistry::new();

    log::info!("Starting PVPC Cheap Backend at http://0.0.0.0:8080");
    log::info!(
        "Available providers: {:?}",
        provider_registry.available_providers()
    );

    HttpServer::new(move || {
        App::new()
            .wrap(Cors::permissive())
            .app_data(web::Data::new(pool.clone()))
            .app_data(web::Data::new(provider_registry.clone()))
            .service(health_check)
            .service(list_providers)
            .configure(api::config)
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}
