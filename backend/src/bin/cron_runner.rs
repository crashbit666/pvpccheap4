//! Cron Runner - Scheduled tasks for PVPC Cheap
//!
//! This binary is meant to be executed by cron or systemd timer.
//! It performs two main tasks:
//! 1. Sync PVPC electricity prices from ESIOS API
//! 2. Run automation rules based on current prices
//!
//! Usage:
//!   cron_runner sync-prices    # Sync today's and tomorrow's prices (smart: skips if already synced)
//!   cron_runner run-automation # Evaluate and execute automation rules (ensures prices exist first)
//!   cron_runner all            # Run both tasks (default)
//!
//! Environment variables:
//!   DATABASE_URL - PostgreSQL connection string (required)
//!   ESIOS_TOKEN  - ESIOS API token for price fetching (required for sync-prices)
//!
//! Recommended cron schedule:
//!   # Sync prices at 20:15 (after tomorrow's prices are published)
//!   15 20 * * * /path/to/cron_runner sync-prices
//!
//!   # Run automation every hour at minute 5
//!   5 * * * * /path/to/cron_runner run-automation

use chrono::Local;
use std::env;
use std::sync::Arc;

// Import from the library crate
use backend::db;
use backend::integrations::ProviderRegistry;
use backend::services::automation_engine::AutomationEngine;
use backend::services::price_fetcher::PriceService;

#[tokio::main]
async fn main() {
    // Initialize logging
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    // Get command from args
    let args: Vec<String> = env::args().collect();
    let command = args.get(1).map(|s| s.as_str()).unwrap_or("all");

    // Initialize database pool
    let database_url = match env::var("DATABASE_URL") {
        Ok(url) => url,
        Err(_) => {
            log::error!("DATABASE_URL environment variable is not set");
            std::process::exit(1);
        }
    };

    let pool = db::init_pool(&database_url);

    match command {
        "sync-prices" => {
            log::info!("Running: sync-prices");
            sync_prices(pool.clone()).await;
        }
        "run-automation" => {
            log::info!("Running: run-automation");
            run_automation(pool.clone()).await;
        }
        "all" => {
            log::info!("Running: all tasks");
            sync_prices(pool.clone()).await;
            run_automation(pool.clone()).await;
        }
        _ => {
            log::error!("Unknown command: {}. Use: sync-prices, run-automation, or all", command);
            std::process::exit(1);
        }
    }

    log::info!("Cron runner completed successfully");
}

async fn sync_prices(pool: db::DbPool) {
    let service = PriceService::new(pool);
    let today = Local::now().date_naive();
    let tomorrow = today + chrono::Duration::days(1);

    // Check if we already have today's prices
    match service.has_prices_for_date(today) {
        Ok(true) => {
            log::info!("Today's prices ({}) already in database, skipping sync", today);
        }
        Ok(false) => {
            log::info!("Today's prices ({}) missing, fetching from ESIOS...", today);
            match service.sync_today().await {
                Ok(count) => log::info!("Synced {} prices for today", count),
                Err(e) => log::error!("Failed to sync today's prices: {}", e),
            }
        }
        Err(e) => {
            log::warn!("Could not check today's prices: {}. Attempting sync anyway...", e);
            match service.sync_today().await {
                Ok(count) => log::info!("Synced {} prices for today", count),
                Err(e) => log::error!("Failed to sync today's prices: {}", e),
            }
        }
    }

    // Check if we already have tomorrow's prices
    match service.has_prices_for_date(tomorrow) {
        Ok(true) => {
            log::info!("Tomorrow's prices ({}) already in database, skipping sync", tomorrow);
        }
        Ok(false) => {
            log::info!("Tomorrow's prices ({}) missing, attempting to fetch from ESIOS...", tomorrow);
            match service.sync_tomorrow().await {
                Ok(count) => log::info!("Synced {} prices for tomorrow", count),
                Err(e) => log::warn!("Could not sync tomorrow's prices (may not be available yet): {}", e),
            }
        }
        Err(e) => {
            log::warn!("Could not check tomorrow's prices: {}. Attempting sync anyway...", e);
            match service.sync_tomorrow().await {
                Ok(count) => log::info!("Synced {} prices for tomorrow", count),
                Err(e) => log::warn!("Could not sync tomorrow's prices: {}", e),
            }
        }
    }
}

/// Ensures we have today's prices before running automation
async fn ensure_today_prices(pool: db::DbPool) -> bool {
    let service = PriceService::new(pool);
    let today = Local::now().date_naive();

    match service.has_prices_for_date(today) {
        Ok(true) => {
            log::debug!("Today's prices available");
            true
        }
        Ok(false) => {
            log::warn!("Today's prices missing! Attempting to fetch before automation...");
            match service.sync_today().await {
                Ok(count) => {
                    log::info!("Synced {} prices for today", count);
                    count >= 24
                }
                Err(e) => {
                    log::error!("Failed to sync today's prices: {}", e);
                    false
                }
            }
        }
        Err(e) => {
            log::error!("Could not check today's prices: {}", e);
            false
        }
    }
}

async fn run_automation(pool: db::DbPool) {
    // First, ensure we have today's prices
    if !ensure_today_prices(pool.clone()).await {
        log::warn!("Running automation without complete price data - some rules may not trigger correctly");
    }

    let registry = Arc::new(ProviderRegistry::new());
    let engine = AutomationEngine::new(pool, registry);

    let results = engine.run().await;

    let successful = results.iter().filter(|r| r.success).count();
    let failed = results.len() - successful;

    log::info!(
        "Automation completed: {} rules executed, {} successful, {} failed",
        results.len(),
        successful,
        failed
    );

    // Log details of failed executions
    for result in results.iter().filter(|r| !r.success) {
        if let Some(ref error) = result.error_message {
            log::error!("Rule {} failed: {}", result.rule_id, error);
        }
    }
}
