//! Cron Runner - Scheduled tasks for PVPC Cheap
//!
//! This binary runs as a daemon with proper cron scheduling:
//! - sync-prices: Runs at startup and daily at 20:30 (when tomorrow's prices are published)
//! - run-automation: Runs at the start of every hour (when prices change)
//! - power-readings: Every 5 minutes, the power of the plugs that are on
//!
//! Environment variables:
//!   DATABASE_URL - PostgreSQL connection string (required)
//!   ESIOS_TOKEN  - ESIOS API token for price fetching (required for sync-prices)

use chrono::Timelike;
use chrono_tz::Europe::Madrid;
use std::env;
use std::sync::{Arc, OnceLock};
use tokio_cron_scheduler::{Job, JobScheduler};

// Import from the library crate
use backend::db::{self, DbPool};
use backend::integrations::ProviderRegistry;
use backend::services::automation_engine::AutomationEngine;
use backend::services::power_monitor;
use backend::services::price_fetcher::{expected_price_rows, PriceService};
use backend::services::schedule_computation::ScheduleComputationService;

/// One registry for every job, so providers keep their connections open
/// between runs (Meross: one MQTT connection per account)
fn registry() -> Arc<ProviderRegistry> {
    static REGISTRY: OnceLock<Arc<ProviderRegistry>> = OnceLock::new();
    REGISTRY.get_or_init(|| Arc::new(ProviderRegistry::new())).clone()
}

#[tokio::main]
async fn main() {
    // Initialize logging
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    // Device actions need to decrypt the provider credentials
    backend::services::credentials::ensure_credentials_key();

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
    let mut sched = JobScheduler::new().await.expect("Failed to create scheduler");

    // Schedule sync-prices at 20:30 every day (Madrid timezone)
    // Cron: "0 30 20 * * *" = second 0, minute 30, hour 20, every day
    let pool_sync = pool.clone();
    let sync_job = Job::new_async_tz("0 30 20 * * *", Madrid, move |_uuid, _l| {
        let pool = pool_sync.clone();
        Box::pin(async move {
            log::info!("Scheduled sync-prices triggered (20:30 Madrid)");
            sync_prices_daily(pool).await;
        })
    })
    .expect("Failed to create sync-prices job");
    sched.add(sync_job).await.expect("Failed to add sync job");

    // Schedule run-automation at the start of every hour (Madrid timezone)
    // Cron: "0 0 * * * *" = second 0, minute 0, every hour
    let pool_auto = pool.clone();
    let automation_job = Job::new_async_tz("0 0 * * * *", Madrid, move |_uuid, _l| {
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

    // Schedule retry job every minute (Madrid timezone)
    // Cron: "0 * * * * *" = second 0, every minute
    let pool_retry = pool.clone();
    let retry_job = Job::new_async_tz("0 * * * * *", Madrid, move |_uuid, _l| {
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

    // Read the power of the plugs that are on every 5 minutes, off the :00
    // so it does not compete with the hourly actions
    let pool_power = pool.clone();
    let power_job = Job::new_async_tz("0 2/5 * * * *", Madrid, move |_uuid, _l| {
        let pool = pool_power.clone();
        Box::pin(async move {
            sample_power(pool).await;
        })
    })
    .expect("Failed to create power job");
    sched.add(power_job).await.expect("Failed to add power job");

    // Brevo disables API keys unused for 90 days
    let mail_job = Job::new_async_tz("0 10 5 * * Mon", Madrid, |_uuid, _l| {
        Box::pin(async move {
            check_mail_key().await;
        })
    })
    .expect("Failed to create mail key job");
    sched.add(mail_job).await.expect("Failed to add mail key job");

    // Renewals, cancellations and refunds of store subscriptions
    let pool_subs = pool.clone();
    let subs_job = Job::new_async_tz("0 20 3 * * *", Madrid, move |_uuid, _l| {
        let pool = pool_subs.clone();
        Box::pin(async move {
            backend::api::billing::recheck_subscriptions(&pool).await;
        })
    })
    .expect("Failed to create subscriptions job");
    sched.add(subs_job).await.expect("Failed to add subscriptions job");

    // Subscriptions ending right now, in case a Google Play notification was lost
    let pool_due = pool.clone();
    let due_subs_job = Job::new_async_tz("30 */5 * * * *", Madrid, move |_uuid, _l| {
        let pool = pool_due.clone();
        Box::pin(async move {
            backend::api::billing::recheck_due_subscriptions(&pool).await;
        })
    })
    .expect("Failed to create due subscriptions job");
    sched.add(due_subs_job).await.expect("Failed to add due subscriptions job");

    // Start the scheduler
    sched.start().await.expect("Failed to start scheduler");

    log::info!("Cron scheduler running. Jobs scheduled (Europe/Madrid timezone):");
    log::info!("  - sync-prices: daily at 20:30");
    log::info!("  - run-automation: every hour at :00");
    log::info!("  - retry-failed: every minute");
    log::info!("  - power-readings: every 5 minutes");
    log::info!("  - mail-key: Mondays at 05:10");
    log::info!("  - subscriptions: daily at 03:20, and ending ones every 5 minutes");
    tokio::spawn(check_mail_key());

    // Run until podman stops the container (SIGTERM) or Ctrl+C, instead of
    // being killed after the stop timeout
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .expect("Failed to listen for SIGTERM");
    tokio::select! {
        _ = terminate.recv() => {}
        _ = tokio::signal::ctrl_c() => {}
    }
    log::info!("Stopping the cron scheduler");
    if let Err(e) = sched.shutdown().await {
        log::warn!("Scheduler did not stop cleanly: {}", e);
    }
}

/// Sync prices at startup - loads today's prices and tomorrow's if past 20:30
async fn sync_prices_startup(pool: Arc<DbPool>) {
    let service = PriceService::new((*pool).clone());
    let schedule_service = ScheduleComputationService::new((*pool).clone());
    let now = backend::time::now();
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

    // A crash while sending a command leaves rows in "executing": retry them
    match schedule_service.reset_interrupted_executions() {
        Ok(count) if count > 0 => log::info!("Re-queued {} interrupted executions", count),
        Ok(_) => {}
        Err(e) => log::warn!("Failed to re-queue interrupted executions: {}", e),
    }

    // Computes schedules and runs the current hour
    log::info!("Running initial automation check...");
    run_scheduled_automation(pool).await;
}

/// Daily sync at 20:30 - fetches tomorrow's prices and computes schedules
/// Includes retry mechanism with exponential backoff if prices aren't available yet
async fn sync_prices_daily(pool: Arc<DbPool>) {
    let service = PriceService::new((*pool).clone());
    let schedule_service = ScheduleComputationService::new((*pool).clone());
    let tomorrow = backend::time::today() + chrono::Duration::days(1);

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
            Ok(count) if count as i64 >= expected_price_rows(tomorrow) => {
                log::info!("Synced {} prices for tomorrow", count);
                success = true;

                // Tomorrow's windows and tonight's overnight windows can now be computed
                if let Err(e) = schedule_service.refresh_all() {
                    log::error!("Failed to compute schedules: {}", e);
                }
            }
            Ok(count) => {
                log::warn!(
                    "Only got {} prices for tomorrow (expected {}), will retry...",
                    count,
                    expected_price_rows(tomorrow)
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

/// Run scheduled automation for the current hour
async fn run_scheduled_automation(pool: Arc<DbPool>) {
    // Catch up if the 20:30 sync failed or the cron was down: fetch missing
    // prices and (re)compute schedules before acting on the current hour
    ensure_prices(&pool).await;
    if let Err(e) = ScheduleComputationService::new((*pool).clone()).refresh_all() {
        log::error!("Failed to compute schedules: {}", e);
    }

    let registry = registry();
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

/// Fetch today's prices, and tomorrow's after 20:30, when they are missing
async fn ensure_prices(pool: &DbPool) {
    let service = PriceService::new(pool.clone());
    let now = backend::time::now();
    let today = now.date_naive();
    let mut dates = vec![today];
    if now.hour() > 20 || (now.hour() == 20 && now.minute() >= 30) {
        dates.push(today + chrono::Duration::days(1));
    }

    for date in dates {
        if service.has_prices_for_date(date).unwrap_or(false) {
            continue;
        }
        log::warn!("Prices for {} missing, fetching from ESIOS...", date);
        match service.sync_prices_for_date(date).await {
            Ok(count) => log::info!("Synced {} prices for {}", count, date),
            Err(e) => log::error!("Failed to sync prices for {}: {}", date, e),
        }
    }
}

/// Power of the plugs that are on, for the measured savings
async fn sample_power(pool: Arc<DbPool>) {
    let registry = registry();
    power_monitor::sample_power(&pool, registry).await;
    // Once a day, at 04:02
    let now = backend::time::now_naive();
    if now.hour() == 4 && now.minute() < 5 {
        match power_monitor::delete_old_readings(&pool) {
            Ok(n) if n > 0 => log::info!("Deleted {} old power readings", n),
            Err(e) => log::warn!("Failed to delete old power readings: {}", e),
            _ => {}
        }
    }
}

/// Retry failed executions that are due for retry
async fn check_mail_key() {
    use backend::services::mailer;
    if !mailer::is_configured() {
        return;
    }
    match mailer::check_key().await {
        Ok(()) => log::info!("Brevo API key works"),
        Err(e) => log::error!("Brevo API key does not work, account emails will fail: {}", e),
    }
}

async fn retry_failed_executions(pool: Arc<DbPool>) {
    let registry = registry();
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
