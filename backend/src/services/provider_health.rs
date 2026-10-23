//! Watches the providers whose cloud API is not official (Meross): it can
//! change or close without notice. When every account that used one in the
//! last hours only failed, the API probably changed. The admin gets an email
//! (ADMIN_EMAIL, or CONTACT_EMAIL), once a day while it lasts and once when it
//! recovers, and the app tells the provider's users how to keep their
//! devices working through an official path: Home Assistant with Matter or
//! HomeKit, which many Meross plugs support.

use crate::db::DbPool;
use crate::schema::{automation_rules, devices, rule_executions, user_integrations};
use chrono::{Duration, NaiveDateTime};
use diesel::prelude::*;
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::Mutex;

/// Providers without an official API
pub const UNOFFICIAL: [&str; 1] = ["meross"];

/// How far back the outcomes count
const WINDOW_HOURS: i64 = 3;
/// Fewer failures than this are bad luck (the cron retries every minute)
const MIN_FAILURES: usize = 3;
/// Emails while it lasts, at most one per this
const REMIND_HOURS: i64 = 24;

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct ProviderStatus {
    pub provider: &'static str,
    pub healthy: bool,
    /// The first failure of the outage, in engine time
    pub failing_since: Option<NaiveDateTime>,
    /// Accounts that used it in the window, and how many of them only failed
    pub accounts: usize,
    pub failing_accounts: usize,
}

/// One device action: (account, success, when)
type Outcome = (i32, bool, NaiveDateTime);

pub fn status(conn: &mut PgConnection, provider: &'static str, now: NaiveDateTime) -> QueryResult<ProviderStatus> {
    let outcomes: Vec<Outcome> = rule_executions::table
        .inner_join(automation_rules::table.inner_join(devices::table.inner_join(user_integrations::table)))
        .filter(user_integrations::provider_name.eq(provider))
        .filter(rule_executions::executed_at.ge(now - Duration::hours(WINDOW_HOURS)))
        .select((user_integrations::id, rule_executions::success, rule_executions::executed_at))
        .load(conn)?;
    Ok(judge(provider, &outcomes))
}

/// Unhealthy when there were enough failures and no account succeeded; with
/// several accounts, at least two must fail (one may just have a wrong password)
fn judge(provider: &'static str, outcomes: &[Outcome]) -> ProviderStatus {
    let mut accounts: BTreeMap<i32, (usize, usize)> = BTreeMap::new();
    for (account, success, _) in outcomes {
        let e = accounts.entry(*account).or_default();
        if *success { e.0 += 1 } else { e.1 += 1 }
    }
    let failing = accounts.values().filter(|(ok, ko)| *ok == 0 && *ko > 0).count();
    let any_success = accounts.values().any(|(ok, _)| *ok > 0);
    let failures: usize = accounts.values().map(|(_, ko)| ko).sum();
    let healthy = any_success || failures < MIN_FAILURES || (accounts.len() > 1 && failing < 2);
    ProviderStatus {
        provider,
        healthy,
        failing_since: (!healthy).then(|| outcomes.iter().filter(|o| !o.1).map(|o| o.2).min()).flatten(),
        accounts: accounts.len(),
        failing_accounts: failing,
    }
}

/// Last email sent per provider: (when, whether it was about an outage)
static ALERTS: Mutex<BTreeMap<&'static str, (NaiveDateTime, bool)>> = Mutex::new(BTreeMap::new());

/// Run by the cron: checks every unofficial provider and emails the admin
pub async fn watch(pool: &DbPool) {
    let now = crate::time::now_naive();
    let statuses: Vec<ProviderStatus> = {
        let Ok(mut conn) = pool.get() else { return };
        UNOFFICIAL.iter().filter_map(|p| status(&mut conn, p, now).ok()).collect()
    };
    for s in statuses {
        let message = {
            let mut alerts = ALERTS.lock().unwrap_or_else(|e| e.into_inner());
            let last = alerts.get(s.provider).copied();
            let due = match (s.healthy, last) {
                (false, None) => true,
                (false, Some((at, outage))) => !outage || now - at >= Duration::hours(REMIND_HOURS),
                (true, Some((_, true))) => true,
                (true, _) => false,
            };
            if !due {
                continue;
            }
            alerts.insert(s.provider, (now, !s.healthy));
            alert_text(&s)
        };
        if s.healthy {
            log::info!("{} works again", s.provider);
        } else {
            log::error!("{} seems down or changed: {:?}", s.provider, s);
        }
        let to = std::env::var("ADMIN_EMAIL").or_else(|_| std::env::var("CONTACT_EMAIL")).unwrap_or_default();
        if to.trim().is_empty() || !crate::services::mailer::is_configured() {
            continue;
        }
        if let Err(e) = crate::services::mailer::send(to.trim(), &message.0, &message.1).await {
            log::warn!("Could not email the {} alert: {}", s.provider, e);
        }
    }
}

fn alert_text(s: &ProviderStatus) -> (String, String) {
    if s.healthy {
        return (format!("PVPC Cheap: {} torna a funcionar", s.provider), format!("Les accions amb {} tornen a anar bé.", s.provider));
    }
    let since = s.failing_since.map(|t| t.format("%d/%m %H:%M").to_string()).unwrap_or_default();
    (
        format!("PVPC Cheap: {} no respon", s.provider),
        format!(
            "Des de {} totes les accions amb {} fallen ({} de {} comptes). Pot ser que \
             hagin canviat la seva API no oficial. Revisa els registres del cron \
             (journalctl --user -u pvp-cron) i l'app avisa els usuaris afectats.",
            since, s.provider, s.failing_accounts, s.accounts
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn at(h: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, 24).unwrap().and_hms_opt(h, 0, 0).unwrap()
    }

    #[test]
    fn test_judge() {
        let s = |o: &[Outcome]| judge("meross", o);
        assert!(s(&[]).healthy);
        // Two failures are not enough
        assert!(s(&[(1, false, at(1)), (1, false, at(2))]).healthy);
        // One account, only failures
        let down = s(&[(1, false, at(1)), (1, false, at(2)), (1, false, at(3))]);
        assert!(!down.healthy);
        assert_eq!(down.failing_since, Some(at(1)));
        // Another account still works
        assert!(s(&[(1, false, at(1)), (1, false, at(2)), (1, false, at(3)), (2, true, at(3))]).healthy);
        // Of several accounts, only one fails: its own problem
        assert!(s(&[(1, false, at(1)), (1, false, at(2)), (1, false, at(3)), (2, false, at(3)), (2, true, at(2))]).healthy);
        // Two accounts that only fail
        assert!(!s(&[(1, false, at(1)), (1, false, at(2)), (2, false, at(3))]).healthy);
        // A success after the failures: it recovered
        assert!(s(&[(1, false, at(1)), (1, false, at(2)), (1, false, at(3)), (1, true, at(4))]).healthy);
    }
}
