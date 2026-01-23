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
use backend::services::schedule_computation::ScheduleComputationService;

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
            run_scheduled_automation(pool).await;
        })
    })
    .expect("Failed to create automation job");
    sched
        .add(automation_job)
        .await
        .expect("Failed to add automation job");

    // Schedule retry job every minute
    // Cron: "0 * * * * *" = second 0, every minute
    let pool_retry = pool.clone();
    let retry_job = Job::new_async("0 * * * * *", move |_uuid, _l| {
        let pool = pool_retry.clone();
        Box::pin(async move {
            retry_failed_executions(pool).await;
        })
    })
    .expect("Failed to create retry job");
    sched
        .add(retry_job)
        .await
        .expect("Failed to add retry job");

    // Start the scheduler
    sched.start().await.expect("Failed to start scheduler");

    log::info!("Cron scheduler running. Jobs scheduled:");
    log::info!("  - sync-prices: daily at 20:30");
    log::info!("  - run-automation: every hour at :00");
    log::info!("  - retry-failed: every minute");

    // Keep the process running
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
    }
}

/// Sync prices at startup - loads today's prices and tomorrow's if past 20:30
async fn sync_prices_startup(pool: Arc<DbPool>) {
    let service = PriceService::new((*pool).clone());
    let schedule_service = ScheduleComputationService::new((*pool).clone());
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
    let mut have_tomorrow_prices = false;
    if now.hour() > 20 || (now.hour() == 20 && now.minute() >= 30) {
        match service.has_prices_for_date(tomorrow) {
            Ok(true) => {
                log::info!("Tomorrow's prices ({}) already in database", tomorrow);
                have_tomorrow_prices = true;
            }
            Ok(false) => {
                log::info!(
                    "Tomorrow's prices ({}) missing and it's past 20:30, fetching...",
                    tomorrow
                );
                match service.sync_tomorrow().await {
                    Ok(count) => {
                        log::info!("Synced {} prices for tomorrow", count);
                        have_tomorrow_prices = count >= 24;
                    }
                    Err(e) => log::warn!("Could not sync tomorrow's prices: {}", e),
                }
            }
            Err(e) => {
                log::warn!("Could not check tomorrow's prices: {}", e);
            }
        }
    }

    // Compute schedules for today
    log::info!("Computing schedules for today...");
    match schedule_service.compute_schedule_for_date(today) {
        Ok(count) => log::info!("Computed {} scheduled executions for today", count),
        Err(e) => log::error!("Failed to compute today's schedules: {}", e),
    }

    // If we have tomorrow's prices, also recompute overnight rules
    if have_tomorrow_prices {
        log::info!("Recomputing overnight rules with complete price data...");
        match schedule_service.recompute_overnight_rules() {
            Ok(count) if count > 0 => {
                log::info!("Recomputed {} overnight scheduled executions", count)
            }
            Ok(_) => log::info!("No overnight rules needed recomputation"),
            Err(e) => log::error!("Failed to recompute overnight rules: {}", e),
        }
    }

    // Mark any missed hours
    match schedule_service.mark_missed_hours() {
        Ok(count) if count > 0 => log::info!("Marked {} hours as missed", count),
        _ => {}
    }

    // Run scheduled automation for current hour
    log::info!("Running initial automation check...");
    run_scheduled_automation(pool).await;
}

/// Daily sync at 20:30 - fetches tomorrow's prices and computes schedules
/// Includes retry mechanism with exponential backoff if prices aren't available yet
async fn sync_prices_daily(pool: Arc<DbPool>) {
    let service = PriceService::new((*pool).clone());
    let schedule_service = ScheduleComputationService::new((*pool).clone());
    let tomorrow = Local::now().date_naive() + chrono::Duration::days(1);

    log::info!("Daily sync: fetching tomorrow's prices ({})...", tomorrow);

    // Retry configuration: max 5 retries with exponential backoff
    // Delays: 2min, 4min, 8min, 16min, 32min (total ~1 hour of retries)
    const MAX_RETRIES: u32 = 5;
    const INITIAL_DELAY_SECS: u64 = 120; // 2 minutes

    let mut attempt = 0;
    let mut success = false;

    while attempt <= MAX_RETRIES && !success {
        if attempt > 0 {
            let delay_secs = INITIAL_DELAY_SECS * (1 << (attempt - 1)); // Exponential backoff
            log::info!(
                "Retry {} of {} for tomorrow's prices in {} seconds...",
                attempt,
                MAX_RETRIES,
                delay_secs
            );
            tokio::time::sleep(tokio::time::Duration::from_secs(delay_secs)).await;
        }

        match service.sync_tomorrow().await {
            Ok(count) if count >= 24 => {
                log::info!("Synced {} prices for tomorrow", count);
                success = true;

                // Compute schedules for tomorrow now that we have prices
                log::info!("Computing schedules for tomorrow...");
                match schedule_service.compute_schedule_for_date(tomorrow) {
                    Ok(sched_count) => {
                        log::info!("Computed {} scheduled executions for tomorrow", sched_count)
                    }
                    Err(e) => log::error!("Failed to compute tomorrow's schedules: {}", e),
                }

                // Recompute overnight rules that span today and tomorrow
                // Now that we have tomorrow's prices, we can calculate the full window
                log::info!("Recomputing overnight rules with complete price data...");
                match schedule_service.recompute_overnight_rules() {
                    Ok(count) if count > 0 => {
                        log::info!("Recomputed {} overnight scheduled executions", count)
                    }
                    Ok(_) => log::info!("No overnight rules needed recomputation"),
                    Err(e) => log::error!("Failed to recompute overnight rules: {}", e),
                }
            }
            Ok(count) => {
                log::warn!(
                    "Only got {} prices for tomorrow (expected 24), will retry...",
                    count
                );
                attempt += 1;
            }
            Err(e) => {
                log::warn!("Failed to sync tomorrow's prices: {}, will retry...", e);
                attempt += 1;
            }
        }
    }

    if !success {
        log::error!(
            "Failed to sync tomorrow's prices after {} retries. \
             Overnight rules may not have complete scheduling.",
            MAX_RETRIES
        );
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

/// Run scheduled automation for the current hour
async fn run_scheduled_automation(pool: Arc<DbPool>) {
    let registry = Arc::new(ProviderRegistry::new());
    let engine = AutomationEngine::new((*pool).clone(), registry);

    let results = engine.execute_current_hour().await;

    let successful = results.iter().filter(|r| r.success).count();
    let failed = results.len() - successful;

    if results.is_empty() {
        log::info!("Scheduled automation: no executions for current hour");
    } else {
        log::info!(
            "Scheduled automation completed: {} executions, {} successful, {} failed",
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

/// Retry failed executions that are due for retry
async fn retry_failed_executions(pool: Arc<DbPool>) {
    let registry = Arc::new(ProviderRegistry::new());
    let engine = AutomationEngine::new((*pool).clone(), registry);
    let schedule_service = ScheduleComputationService::new((*pool).clone());

    // First, mark any missed hours (hours that have passed without execution)
    match schedule_service.mark_missed_hours() {
        Ok(count) if count > 0 => log::info!("Marked {} hours as missed", count),
        Err(e) => log::warn!("Failed to mark missed hours: {}", e),
        _ => {}
    }

    // Then retry failed executions
    let results = engine.retry_failed_executions().await;

    if !results.is_empty() {
        let successful = results.iter().filter(|r| r.success).count();
        let failed = results.len() - successful;

        log::info!(
            "Retry completed: {} retries, {} successful, {} still failing",
            results.len(),
            successful,
            failed
        );

        for result in results.iter().filter(|r| !r.success) {
            if let Some(ref error) = result.error_message {
                log::warn!("Retry for rule {} failed: {}", result.rule_id, error);
            }
        }
    }
}
