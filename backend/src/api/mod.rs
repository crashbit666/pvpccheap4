use actix_web::web;

pub mod auth;
pub mod automation;
pub mod devices;
pub mod integrations;
pub mod prices;
pub mod rules;
pub mod schedules;

pub fn config(cfg: &mut web::ServiceConfig) {
    // Auth routes (public)
    cfg.service(
        web::scope("/api/auth")
            .service(auth::register)
            .service(auth::login),
    );

    // Integration routes (protected)
    cfg.service(
        web::scope("/api/integrations")
            .service(integrations::add_integration)
            .service(integrations::list_integrations)
            .service(integrations::delete_integration),
    );

    // Device routes (protected)
    cfg.service(
        web::scope("/api/devices")
            .service(devices::list_devices)
            .service(devices::sync_devices)
            .service(devices::control_device)
            .service(devices::get_device_state)
            .service(devices::update_device)
            .service(devices::delete_device),
    );

    // Automation rules routes (protected)
    cfg.service(
        web::scope("/api/rules")
            .service(rules::list_rules)
            .service(rules::get_rule)
            .service(rules::create_rule)
            .service(rules::update_rule)
            .service(rules::delete_rule)
            .service(rules::toggle_rule)
            .service(rules::get_rule_executions),
    );

    // Automation engine routes (protected)
    cfg.service(
        web::scope("/api/automation")
            .service(automation::run_automation),
    );

    // Schedule routes (protected)
    cfg.service(
        web::scope("/api/schedules")
            .service(schedules::get_schedule),
    );

    // Price routes (public - no auth required for price info)
    cfg.service(
        web::scope("/api/prices")
            .service(prices::get_prices)
            .service(prices::get_current_price)
            .service(prices::get_price_summary)
            .service(prices::get_cheapest_hours)
            .service(prices::get_expensive_hours)
            .service(prices::sync_prices)
            .service(prices::sync_prices_for_date),
    );
}
