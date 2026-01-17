use actix_web::web;

pub mod auth;
pub mod automation;
pub mod devices;
pub mod integrations;
pub mod rules;

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
            .service(integrations::list_integrations),
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
}
