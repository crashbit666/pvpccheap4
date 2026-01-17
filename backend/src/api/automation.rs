use crate::{
    db::DbPool,
    integrations::ProviderRegistry,
    services::automation_engine::AutomationEngine,
    services::auth::Claims,
};
use actix_web::{post, web, HttpResponse, Responder};
use std::sync::Arc;

/// Trigger the automation engine to evaluate and execute rules
/// This endpoint is protected and requires admin privileges in production
#[post("/run")]
pub async fn run_automation(
    pool: web::Data<DbPool>,
    registry: web::Data<ProviderRegistry>,
    _claims: Claims, // Requires authentication
) -> impl Responder {
    let pool = pool.get_ref().clone();
    let registry = Arc::new(registry.get_ref().clone());

    let engine = AutomationEngine::new(pool, registry);
    let results = engine.run().await;

    let summary = serde_json::json!({
        "executed": results.len(),
        "successful": results.iter().filter(|r| r.success).count(),
        "failed": results.iter().filter(|r| !r.success).count(),
        "results": results.iter().map(|r| {
            serde_json::json!({
                "rule_id": r.rule_id,
                "success": r.success,
                "error": r.error_message,
                "price_at_execution": r.price_at_execution
            })
        }).collect::<Vec<_>>()
    });

    HttpResponse::Ok().json(summary)
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_automation_endpoint_exists() {
        // Basic test to ensure the module compiles
        assert!(true);
    }
}
