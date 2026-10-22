use actix_cors::Cors;
use actix_web::{get, web, App, HttpResponse, HttpServer, Responder};

mod api;
mod db;
mod integrations;
mod models;
mod schema;
mod services;
mod time;

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

    // Refuse to start without a real JWT signing secret
    services::auth::ensure_jwt_secret();
    // ...nor without the key that encrypts the provider credentials
    services::credentials::ensure_credentials_key();

    // DB Pool initialization
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = db::init_pool(&database_url);
    if let Ok(mut conn) = pool.get() {
        if let Err(e) = services::credentials::encrypt_legacy_rows(&mut conn) {
            log::error!("Could not encrypt stored credentials: {}", e);
        }
    }

    // Provider registry
    let provider_registry = ProviderRegistry::new();

    log::info!("Starting PVPC Cheap Backend at http://0.0.0.0:8080");
    log::info!(
        "Available providers: {:?}",
        provider_registry.available_providers()
    );

    // Shared by the workers: the daily allowance of rules written in plain words
    let draft_limit = web::Data::new(backend::services::rule_assistant::DailyLimit::default());

    HttpServer::new(move || {
        App::new()
            .wrap(Cors::permissive())
            .app_data(web::Data::new(pool.clone()))
            .app_data(web::Data::new(provider_registry.clone()))
            .app_data(draft_limit.clone())
            .service(health_check)
            .service(list_providers)
            .configure(api::config)
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}
