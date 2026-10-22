use actix_web::web;

pub mod account_pages;
pub mod alerts;
pub mod auth;
pub mod billing;
pub mod automation;
pub mod devices;
pub mod integrations;
pub mod legal;
pub mod prices;
pub mod rules;
pub mod savings;
pub mod schedules;

pub fn config(cfg: &mut web::ServiceConfig) {
    // Privacy policy, terms and account deletion (public web pages)
    cfg.service(legal::privacy)
        .service(legal::terms)
        .service(legal::delete_account)
        .service(account_pages::verify_email)
        .service(account_pages::reset_password_form)
        .service(account_pages::reset_password);

    // Auth routes (public)
    cfg.service(
        web::scope("/api/auth")
            .service(auth::register)
            .service(auth::login)
            .service(auth::me)
            .service(auth::change_email)
            .service(auth::change_zone)
            .service(auth::forgot_password)
            .service(auth::change_password)
            .service(auth::delete_account),
    );

    // Store purchases of the paid plan (protected)
    cfg.service(web::scope("/api/billing").service(billing::google_purchase));

    // Failed device actions (protected)
    cfg.service(web::scope("/api/alerts").service(alerts::list_alerts));

    // What the rules saved (protected)
    cfg.service(web::scope("/api/savings").service(savings::get_savings));

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
            .service(devices::get_device_usage)
            .service(devices::update_device)
            .service(devices::delete_device),
    );

    // Automation rules routes (protected)
    cfg.service(
        web::scope("/api/rules")
            .service(rules::list_rules)
            .service(rules::get_rule)
            .service(rules::create_rule)
            .service(rules::draft_rule)
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

    // Price routes (reads are public, /sync requires auth)
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
