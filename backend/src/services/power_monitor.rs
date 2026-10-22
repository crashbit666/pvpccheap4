//! Reads the power of the plugs that are on, every few minutes, so the
//! savings use what the appliance really drew instead of a typed power.

use crate::{
    db::DbPool,
    integrations::ProviderRegistry,
    schema::{devices, power_readings, user_integrations},
    services::credentials,
};
use chrono::Duration;
use diesel::prelude::*;
use log::{debug, info, warn};
use std::sync::Arc;

/// Readings older than this are deleted
const KEEP_DAYS: i64 = 400;

/// (device id, external id, is on, measures power, provider, stored credentials)
type Candidate = (i32, String, bool, Option<bool>, String, String);

/// Asks the plugs not asked yet whether they measure power, and stores the
/// power of the ones that do and are on.
pub async fn sample_power(pool: &DbPool, registry: Arc<ProviderRegistry>) {
    let candidates: Vec<Candidate> = {
        let Ok(mut conn) = pool.get() else { return };
        match devices::table
            .inner_join(user_integrations::table)
            .filter(user_integrations::is_active.eq(true))
            .filter(
                devices::measures_power
                    .is_null()
                    .or(devices::measures_power.eq(true).and(devices::is_on.eq(true))),
            )
            .select((
                devices::id,
                devices::external_id,
                devices::is_on,
                devices::measures_power,
                user_integrations::provider_name,
                user_integrations::credentials_json,
            ))
            .load(&mut conn)
        {
            Ok(rows) => rows,
            Err(e) => {
                warn!("Failed to load devices for power readings: {}", e);
                return;
            }
        }
    };

    for (device_id, external_id, is_on, measures, provider_name, stored) in candidates {
        let Some(provider) = registry.get(&provider_name) else { continue };
        let Ok(creds) = credentials::decrypt(&stored) else { continue };

        let measures = match measures {
            Some(m) => m,
            None => match provider.supports_power_reading(&creds, &external_id).await {
                Ok(m) => {
                    info!("Device {} measures power: {}", device_id, m);
                    if let Ok(mut conn) = pool.get() {
                        let _ = diesel::update(devices::table.find(device_id))
                            .set(devices::measures_power.eq(m))
                            .execute(&mut conn);
                    }
                    m
                }
                // Unreachable now (offline, expired session): ask next time
                Err(e) => {
                    debug!("Could not ask device {} for its abilities: {}", device_id, e);
                    continue;
                }
            },
        };
        if !measures || !is_on {
            continue;
        }

        match provider.read_power(&creds, &external_id).await {
            Ok(watts) => {
                if let Ok(mut conn) = pool.get() {
                    let _ = diesel::insert_into(power_readings::table)
                        .values((
                            power_readings::device_id.eq(device_id),
                            power_readings::measured_at.eq(crate::time::now_naive()),
                            power_readings::watts.eq(watts),
                        ))
                        .execute(&mut conn);
                }
            }
            Err(e) => debug!("Could not read the power of device {}: {}", device_id, e),
        }
    }
}

/// Deletes readings older than KEEP_DAYS
pub fn delete_old_readings(pool: &DbPool) -> Result<usize, String> {
    let mut conn = pool.get().map_err(|e| e.to_string())?;
    let limit = crate::time::now_naive() - Duration::days(KEEP_DAYS);
    diesel::delete(power_readings::table.filter(power_readings::measured_at.lt(limit)))
        .execute(&mut conn)
        .map_err(|e| e.to_string())
}
