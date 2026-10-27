//! Device actions that failed and were not fixed by the automatic retries,
//! so the app can warn the user (e.g. "the boiler could not be turned off").

use crate::{
    db::DbPool,
    schema::{automation_rules, devices, rule_executions},
    services::auth::Claims,
};
use actix_web::{HttpResponse, Responder, get, web};
use chrono::{Duration, NaiveDateTime};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

/// Retries run every minute up to 5 times: a failure younger than this may
/// still be fixed, so it is not reported yet.
const RETRY_GRACE_MINUTES: i64 = 6;
/// How far back an alert can go
const MAX_LOOKBACK_HOURS: i64 = 48;

#[derive(Deserialize)]
pub struct AlertsQuery {
    /// Only failures after this local time (YYYY-MM-DDTHH:MM:SS)
    pub since: Option<NaiveDateTime>,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct Alert {
    pub execution_id: i32,
    pub rule_id: i32,
    pub rule_name: String,
    pub device_name: String,
    pub action: String,
    pub failed_at: NaiveDateTime,
    pub error: Option<String>,
}

#[get("")]
pub async fn list_alerts(
    pool: web::Data<DbPool>,
    claims: Claims,
    query: web::Query<AlertsQuery>,
) -> impl Responder {
    let Ok(user_id) = claims.sub.parse::<i32>() else {
        return HttpResponse::BadRequest().body("Invalid user ID");
    };
    let Ok(mut conn) = pool.get() else {
        return HttpResponse::InternalServerError().body("Database connection error");
    };

    // The app sees local times: `since` and `failed_at` are in the user's zone
    let zone = crate::zones::of_user(&mut conn, user_id);
    let now = crate::time::now_naive();
    let floor = now - Duration::hours(MAX_LOOKBACK_HOURS);
    let since = query.since.map_or(floor, |s| zone.to_engine(s).max(floor));
    let settled = now - Duration::minutes(RETRY_GRACE_MINUTES);

    // Every execution of the user's rules since then, oldest first
    let rows: Vec<(
        i32,
        i32,
        String,
        String,
        String,
        bool,
        NaiveDateTime,
        Option<String>,
    )> = match rule_executions::table
        .inner_join(automation_rules::table.inner_join(devices::table))
        .filter(automation_rules::user_id.eq(user_id))
        .filter(rule_executions::executed_at.gt(since))
        .order(rule_executions::executed_at.asc())
        .select((
            rule_executions::id,
            rule_executions::rule_id,
            automation_rules::name,
            devices::name,
            rule_executions::action_taken,
            rule_executions::success,
            rule_executions::executed_at,
            rule_executions::error_message,
        ))
        .load(&mut conn)
    {
        Ok(r) => r,
        Err(_) => return HttpResponse::InternalServerError().body("Error loading executions"),
    };

    let alerts: Vec<Alert> = unresolved_failures(rows, settled)
        .into_iter()
        .map(|a| Alert {
            failed_at: zone.to_local(a.failed_at),
            ..a
        })
        .collect();
    HttpResponse::Ok().json(alerts)
}

/// The last failure of each rule that no later success fixed and that is
/// older than `settled`. `rows` must be sorted by time.
fn unresolved_failures(
    rows: Vec<(
        i32,
        i32,
        String,
        String,
        String,
        bool,
        NaiveDateTime,
        Option<String>,
    )>,
    settled: NaiveDateTime,
) -> Vec<Alert> {
    let mut pending: std::collections::BTreeMap<i32, Alert> = Default::default();
    for (id, rule_id, rule_name, device_name, action, success, at, error) in rows {
        if success {
            pending.remove(&rule_id);
        } else {
            pending.insert(
                rule_id,
                Alert {
                    execution_id: id,
                    rule_id,
                    rule_name,
                    device_name,
                    action,
                    failed_at: at,
                    error,
                },
            );
        }
    }
    let mut alerts: Vec<Alert> = pending
        .into_values()
        .filter(|a| a.failed_at <= settled)
        .collect();
    alerts.sort_by_key(|a| a.failed_at);
    alerts
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn at(h: u32, m: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, 21)
            .unwrap()
            .and_hms_opt(h, m, 0)
            .unwrap()
    }

    fn row(
        id: i32,
        rule: i32,
        ok: bool,
        t: NaiveDateTime,
    ) -> (
        i32,
        i32,
        String,
        String,
        String,
        bool,
        NaiveDateTime,
        Option<String>,
    ) {
        (
            id,
            rule,
            format!("r{}", rule),
            "plug".into(),
            "turn_off".into(),
            ok,
            t,
            (!ok).then(|| "timeout".into()),
        )
    }

    #[test]
    fn test_failure_fixed_by_retry_is_not_reported() {
        let rows = vec![
            row(1, 6, false, at(1, 0)),
            row(2, 6, false, at(1, 1)),
            row(3, 6, true, at(1, 4)),
        ];
        assert!(unresolved_failures(rows, at(2, 0)).is_empty());
    }

    #[test]
    fn test_persisting_failure_is_reported_once() {
        let rows = vec![
            row(1, 6, false, at(1, 0)),
            row(2, 6, false, at(1, 1)),
            row(3, 13, true, at(1, 2)),
        ];
        let alerts = unresolved_failures(rows, at(2, 0));
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].execution_id, 2);
        assert_eq!(alerts[0].rule_id, 6);
    }

    #[test]
    fn test_recent_failure_waits_for_retries() {
        let rows = vec![row(1, 6, false, at(1, 0))];
        assert!(unresolved_failures(rows, at(0, 59)).is_empty());
    }
}
