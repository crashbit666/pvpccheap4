//! Cron Runner - Scheduled tasks for PVPC Cheap
//!
//! This binary runs as a daemon with proper cron scheduling:
//! - sync-prices: Runs at startup and daily at 20:30 (when tomorrow's prices are published)
//! - run-automation: Runs at the start of every hour (when prices change)
//!
//! Environment variables:
//!   DATABASE_URL - PostgreSQL connection string (required)
//!   ESIOS_TOKEN  - ESIOS API token for price fetching (required for sync-prices)

use chrono::{Local, Timelike};
use std::env;
use std::sync::Arc;
use tokio_cron_scheduler::{Job, JobScheduler};

// Import from the library crate
use backend::db::{self, DbPool};
use backend::integrations::ProviderRegistry;
use backend::services::automation_engine::AutomationEngine;
use backend::services::price_fetcher::PriceService;

#[tokio::main]
async fn main() {
    // Initialize logging
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    // Initialize database pool
    let database_url = match env::var("DATABASE_URL") {
        Ok(url) => url,
        Err(_) => {
            log::error!("DATABASE_URL environment variable is not set");
            std::process::exit(1);
        }
    };

    let pool = Arc::new(db::init_pool(&database_url));

    log::info!("Starting PVPC Cheap cron scheduler...");

    // Run initial sync at startup
    sync_prices_startup(pool.clone()).await;

    // Create scheduler
    let sched = JobScheduler::new().await.expect("Failed to create scheduler");

    // Schedule sync-prices at 20:30 every day
    // Cron: "0 30 20 * * *" = second 0, minute 30, hour 20, every day
    let pool_sync = pool.clone();
    let sync_job = Job::new_async("0 30 20 * * *", move |_uuid, _l| {
        let pool = pool_sync.clone();
        Box::pin(async move {
            log::info!("Scheduled sync-prices triggered (20:30)");
            sync_prices_daily(pool).await;
        })
    })
    .expect("Failed to create sync-prices job");
    sched.add(sync_job).await.expect("Failed to add sync job");

    // Schedule run-automation at the start of every hour
    // Cron: "0 0 * * * *" = second 0, minute 0, every hour
    let pool_auto = pool.clone();
    let automation_job = Job::new_async("0 0 * * * *", move |_uuid, _l| {
        let pool = pool_auto.clone();
        Box::pin(async move {
            log::info!("Scheduled run-automation triggered (hourly)");
            run_automation(pool).await;
        })
    })
    .expect("Failed to create automation job");
    sched
        .add(automation_job)
        .await
        .expect("Failed to add automation job");

    // Start the scheduler
    sched.start().await.expect("Failed to start scheduler");

    log::info!("Cron scheduler running. Jobs scheduled:");
    log::info!("  - sync-prices: daily at 20:30");
    log::info!("  - run-automation: every hour at :00");

    // Keep the process running
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
    }
}

/// Sync prices at startup - loads today's prices and tomorrow's if past 20:30
async fn sync_prices_startup(pool: Arc<DbPool>) {
    let service = PriceService::new((*pool).clone());
    let now = Local::now();
    let today = now.date_naive();
    let tomorrow = today + chrono::Duration::days(1);

    log::info!("Startup sync: checking prices...");

    // Always try to sync today's prices if missing
    match service.has_prices_for_date(today) {
        Ok(true) => {
            log::info!("Today's prices ({}) already in database", today);
        }
        Ok(false) => {
            log::info!("Today's prices ({}) missing, fetching from ESIOS...", today);
            match service.sync_today().await {
                Ok(count) => log::info!("Synced {} prices for today", count),
                Err(e) => log::error!("Failed to sync today's prices: {}", e),
            }
        }
        Err(e) => {
            log::warn!("Could not check today's prices: {}", e);
            if let Err(e) = service.sync_today().await {
                log::error!("Failed to sync today's prices: {}", e);
            }
        }
    }

    // If it's past 20:30, also try to sync tomorrow's prices
    if now.hour() > 20 || (now.hour() == 20 && now.minute() >= 30) {
        match service.has_prices_for_date(tomorrow) {
            Ok(true) => {
                log::info!("Tomorrow's prices ({}) already in database", tomorrow);
            }
            Ok(false) => {
                log::info!(
                    "Tomorrow's prices ({}) missing and it's past 20:30, fetching...",
                    tomorrow
                );
                match service.sync_tomorrow().await {
                    Ok(count) => log::info!("Synced {} prices for tomorrow", count),
                    Err(e) => log::warn!("Could not sync tomorrow's prices: {}", e),
                }
            }
            Err(e) => {
                log::warn!("Could not check tomorrow's prices: {}", e);
            }
        }
    }

    // Also run automation once at startup to handle current hour
    log::info!("Running initial automation check...");
    run_automation(pool).await;
}

/// Daily sync at 20:30 - fetches tomorrow's prices
async fn sync_prices_daily(pool: Arc<DbPool>) {
    let service = PriceService::new((*pool).clone());
    let tomorrow = Local::now().date_naive() + chrono::Duration::days(1);

    log::info!("Daily sync: fetching tomorrow's prices ({})...", tomorrow);
    match service.sync_tomorrow().await {
        Ok(count) => log::info!("Synced {} prices for tomorrow", count),
        Err(e) => log::error!("Failed to sync tomorrow's prices: {}", e),
    }
}

/// Run automation rules based on current prices
async fn run_automation(pool: Arc<DbPool>) {
    // First, ensure we have today's prices
    let service = PriceService::new((*pool).clone());
    let today = Local::now().date_naive();

    match service.has_prices_for_date(today) {
        Ok(false) => {
            log::warn!("Today's prices missing! Attempting to fetch before automation...");
            if let Err(e) = service.sync_today().await {
                log::error!("Failed to sync today's prices: {}", e);
                log::warn!("Running automation without complete price data");
            }
        }
        Err(e) => {
            log::warn!("Could not check today's prices: {}", e);
        }
        Ok(true) => {}
    }

    let registry = Arc::new(ProviderRegistry::new());
    let engine = AutomationEngine::new((*pool).clone(), registry);

    let results = engine.run().await;

    let successful = results.iter().filter(|r| r.success).count();
    let failed = results.len() - successful;

    if results.is_empty() {
        log::info!("Automation: no rules to execute");
    } else {
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
}
