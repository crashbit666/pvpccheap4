use crate::{
    db::DbPool,
    integrations::ProviderRegistry,
    models::{AutomationRule, NewAutomationRule, RuleExecution},
    schema::{automation_rules, devices, rule_executions, user_integrations},
    services::{
        auth::Claims,
        automation_engine::AutomationEngine,
        schedule_computation::{parse_time_of_day, window_specs, ScheduleComputationService},
    },
};
use std::sync::Arc;
use actix_web::{delete, get, post, put, web, HttpResponse, Responder};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// Check the config keys the schedule computation reads, so a typo is rejected
/// instead of being silently replaced by a default (e.g. the whole day).
fn validate_rule_config(rule_type: &str, config: &JsonValue) -> Result<(), String> {
    if !config.is_object() {
        return Err("config must be a JSON object".to_string());
    }

    if let Some(hours) = config.get("cheapest_hours") {
        match hours.as_i64() {
            Some(1..=24) => {}
            _ => return Err("cheapest_hours must be an integer between 1 and 24".to_string()),
        }
    }

    if let Some(threshold) = config.get("price_threshold") {
        if !threshold.is_number() {
            return Err("price_threshold must be a number (€/kWh)".to_string());
        }
    }

    for key in ["time_range_start", "time_range_end"] {
        if let Some(value) = config.get(key) {
            if value.as_str().and_then(parse_time_of_day).is_none() {
                return Err(format!("{} must be a time in HH:MM format", key));
            }
        }
    }

    if rule_type == "cheapest_hours"
        && config.get("time_range_start").is_some() != config.get("time_range_end").is_some()
    {
        return Err("time_range_start and time_range_end must be set together".to_string());
    }

    validate_windows(config)
}

/// Check the "windows" list: valid times, no overlaps, and per-window hours that fit.
fn validate_windows(config: &JsonValue) -> Result<(), String> {
    const MAX_WINDOWS: usize = 8;

    if let Some(distribution) = config.get("distribution") {
        if !matches!(distribution.as_str(), Some("total") | Some("per_window")) {
            return Err("distribution must be \"total\" or \"per_window\"".to_string());
        }
    }

    let Some(list) = config.get("windows") else {
        return Ok(());
    };
    let Some(list) = list.as_array() else {
        return Err("windows must be a list".to_string());
    };
    if list.len() > MAX_WINDOWS {
        return Err(format!("At most {} windows", MAX_WINDOWS));
    }
    for (i, w) in list.iter().enumerate() {
        for key in ["start", "end"] {
            if w.get(key).and_then(|v| v.as_str()).and_then(parse_time_of_day).is_none() {
                return Err(format!("Window {}: {} must be a time in HH:MM format", i + 1, key));
            }
        }
        if let Some(hours) = w.get("hours") {
            if !matches!(hours.as_i64(), Some(1..=24)) {
                return Err(format!("Window {}: hours must be an integer between 1 and 24", i + 1));
            }
        }
    }

    let specs = window_specs(config);
    let per_window = config.get("distribution").and_then(|d| d.as_str()) == Some("per_window");
    let mut used = [false; 24];
    for (i, spec) in specs.iter().enumerate() {
        let hours = spec.hours_of_day();
        if hours.iter().any(|h| used[*h as usize]) {
            return Err(format!("Window {} overlaps another window", i + 1));
        }
        hours.iter().for_each(|h| used[*h as usize] = true);
        if per_window {
            if let Some(n) = spec.hours.filter(|n| *n > hours.len()) {
                return Err(format!(
                    "Window {} lasts {} h and cannot hold {} h",
                    i + 1,
                    hours.len(),
                    n
                ));
            }
        }
    }

    Ok(())
}

/// Turn off the devices that their rules switched on for the current hour,
/// used before the devices themselves are removed.
pub(crate) async fn release_devices(
    pool: &web::Data<DbPool>,
    registry: &web::Data<ProviderRegistry>,
    device_ids: &[i32],
) {
    let rule_ids: Vec<i32> = match pool.get() {
        Ok(mut conn) => automation_rules::table
            .filter(automation_rules::device_id.eq_any(device_ids))
            .select(automation_rules::id)
            .load(&mut conn)
            .unwrap_or_default(),
        Err(_) => return,
    };
    for rule_id in rule_ids {
        release_device(pool, registry, rule_id).await;
    }
}

/// Turn off a rule's device if the rule switched it on for the current hour,
/// used when the rule stops applying (disabled or about to be deleted).
async fn release_device(pool: &web::Data<DbPool>, registry: &web::Data<ProviderRegistry>, rule_id: i32) {
    let engine = AutomationEngine::new(pool.get_ref().clone(), Arc::new(registry.get_ref().clone()));
    if let Some(result) = engine.release_device_for_rule(rule_id).await {
        if !result.success {
            log::warn!(
                "Could not turn off device of rule {}: {:?}",
                rule_id, result.error_message
            );
        }
    }
}

// ============================================================================
// Request/Response DTOs
// ============================================================================

#[derive(Deserialize)]
pub struct CreateRuleRequest {
    pub device_id: i32,
    pub name: String,
    pub rule_type: String,
    pub action: String,
    pub config: JsonValue,
    #[serde(default)]
    pub is_enabled: Option<bool>,
    #[serde(default)]
    pub priority: Option<i32>,
}

#[derive(Deserialize)]
pub struct UpdateRuleRequest {
    pub name: Option<String>,
    pub rule_type: Option<String>,
    pub action: Option<String>,
    pub config: Option<JsonValue>,
    pub is_enabled: Option<bool>,
    pub priority: Option<i32>,
}

#[derive(Serialize)]
pub struct RuleResponse {
    pub id: i32,
    pub user_id: i32,
    pub device_id: i32,
    pub device_name: String,
    pub name: String,
    pub rule_type: String,
    pub action: String,
    pub config: JsonValue,
    pub is_enabled: bool,
    pub priority: i32,
    pub created_at: String,
    pub updated_at: String,
    pub last_triggered_at: Option<String>,
}

#[derive(Serialize)]
pub struct ExecutionResponse {
    pub id: i32,
    pub rule_id: i32,
    pub executed_at: String,
    pub action_taken: String,
    pub success: bool,
    pub error_message: Option<String>,
    pub price_at_execution: Option<f64>,
}

// ============================================================================
// Endpoints
// ============================================================================

/// List all automation rules for the authenticated user
#[get("")]
pub async fn list_rules(pool: web::Data<DbPool>, claims: Claims) -> impl Responder {
    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return HttpResponse::InternalServerError().body("Database connection error"),
    };

    let user_id: i32 = match claims.sub.parse() {
        Ok(id) => id,
        Err(_) => return HttpResponse::BadRequest().body("Invalid user ID"),
    };

    // Get rules with device names
    let results: Vec<(AutomationRule, String)> = match automation_rules::table
        .inner_join(devices::table)
        .filter(automation_rules::user_id.eq(user_id))
        .select((AutomationRule::as_select(), devices::name))
        .order((automation_rules::priority.asc(), automation_rules::id.asc())) // id breaks ties: stable list
        .load(&mut conn)
    {
        Ok(r) => r,
        Err(_) => return HttpResponse::InternalServerError().body("Error fetching rules"),
    };

    let response: Vec<RuleResponse> = results
        .into_iter()
        .map(|(rule, device_name)| RuleResponse {
            id: rule.id,
            user_id: rule.user_id,
            device_id: rule.device_id,
            device_name,
            name: rule.name,
            rule_type: rule.rule_type,
            action: rule.action,
            config: rule.config,
            is_enabled: rule.is_enabled,
            priority: rule.priority,
            created_at: rule.created_at.to_string(),
            updated_at: rule.updated_at.to_string(),
            last_triggered_at: rule.last_triggered_at.map(|t| t.to_string()),
        })
        .collect();

    HttpResponse::Ok().json(response)
}

/// Get a specific rule by ID
#[get("/{rule_id}")]
pub async fn get_rule(
    pool: web::Data<DbPool>,
    claims: Claims,
    path: web::Path<i32>,
) -> impl Responder {
    let rule_id = path.into_inner();
    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return HttpResponse::InternalServerError().body("Database connection error"),
    };

    let user_id: i32 = match claims.sub.parse() {
        Ok(id) => id,
        Err(_) => return HttpResponse::BadRequest().body("Invalid user ID"),
    };

    let result: Option<(AutomationRule, String)> = automation_rules::table
        .inner_join(devices::table)
        .filter(automation_rules::id.eq(rule_id))
        .filter(automation_rules::user_id.eq(user_id))
        .select((AutomationRule::as_select(), devices::name))
        .first(&mut conn)
        .optional()
        .unwrap_or(None);

    match result {
        Some((rule, device_name)) => {
            let response = RuleResponse {
                id: rule.id,
                user_id: rule.user_id,
                device_id: rule.device_id,
                device_name,
                name: rule.name,
                rule_type: rule.rule_type,
                action: rule.action,
                config: rule.config,
                is_enabled: rule.is_enabled,
                priority: rule.priority,
                created_at: rule.created_at.to_string(),
                updated_at: rule.updated_at.to_string(),
                last_triggered_at: rule.last_triggered_at.map(|t| t.to_string()),
            };
            HttpResponse::Ok().json(response)
        }
        None => HttpResponse::NotFound().body("Rule not found"),
    }
}

/// Create a new automation rule
#[post("")]
pub async fn create_rule(
    pool: web::Data<DbPool>,
    claims: Claims,
    body: web::Json<CreateRuleRequest>,
) -> impl Responder {
    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return HttpResponse::InternalServerError().body("Database connection error"),
    };

    let user_id: i32 = match claims.sub.parse() {
        Ok(id) => id,
        Err(_) => return HttpResponse::BadRequest().body("Invalid user ID"),
    };

    // Validate rule_type
    let valid_types = ["price_threshold", "cheapest_hours", "time_schedule", "manual"];
    if !valid_types.contains(&body.rule_type.as_str()) {
        return HttpResponse::BadRequest().body(format!(
            "Invalid rule_type. Must be one of: {:?}",
            valid_types
        ));
    }

    // Validate action
    let valid_actions = ["turn_on", "turn_off", "toggle"];
    if !valid_actions.contains(&body.action.as_str()) {
        return HttpResponse::BadRequest().body(format!(
            "Invalid action. Must be one of: {:?}",
            valid_actions
        ));
    }

    if let Err(e) = validate_rule_config(&body.rule_type, &body.config) {
        return HttpResponse::BadRequest().body(e);
    }

    // Verify device belongs to user
    let device_belongs_to_user = devices::table
        .inner_join(user_integrations::table)
        .filter(devices::id.eq(body.device_id))
        .filter(user_integrations::user_id.eq(user_id))
        .select(devices::id)
        .first::<i32>(&mut conn)
        .is_ok();

    if !device_belongs_to_user {
        return HttpResponse::NotFound().body("Device not found");
    }

    // Create the rule
    let new_rule = NewAutomationRule {
        user_id,
        device_id: body.device_id,
        name: body.name.clone(),
        rule_type: body.rule_type.clone(),
        action: body.action.clone(),
        config: body.config.clone(),
        is_enabled: body.is_enabled.unwrap_or(true),
        priority: body.priority.unwrap_or(100),
    };

    match diesel::insert_into(automation_rules::table)
        .values(&new_rule)
        .get_result::<AutomationRule>(&mut conn)
    {
        Ok(rule) => {
            // Compute schedules for this new rule; a pick for the current hour is
            // executed by the cron's per-minute job
            let schedule_service = ScheduleComputationService::new(pool.get_ref().clone());
            if let Err(e) = schedule_service.refresh_rule(rule.id) {
                log::warn!("Failed to compute schedule for new rule {}: {}", rule.id, e);
            }

            HttpResponse::Created().json(rule)
        }
        Err(e) => {
            HttpResponse::InternalServerError().body(format!("Failed to create rule: {}", e))
        }
    }
}

/// Update an existing rule
#[put("/{rule_id}")]
pub async fn update_rule(
    pool: web::Data<DbPool>,
    registry: web::Data<ProviderRegistry>,
    claims: Claims,
    path: web::Path<i32>,
    body: web::Json<UpdateRuleRequest>,
) -> impl Responder {
    let rule_id = path.into_inner();
    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return HttpResponse::InternalServerError().body("Database connection error"),
    };

    let user_id: i32 = match claims.sub.parse() {
        Ok(id) => id,
        Err(_) => return HttpResponse::BadRequest().body("Invalid user ID"),
    };

    // Verify rule exists and belongs to user
    let current_rule_type = match automation_rules::table
        .filter(automation_rules::id.eq(rule_id))
        .filter(automation_rules::user_id.eq(user_id))
        .select(automation_rules::rule_type)
        .first::<String>(&mut conn)
    {
        Ok(t) => t,
        Err(_) => return HttpResponse::NotFound().body("Rule not found"),
    };

    // Validate rule_type if provided
    if let Some(ref rule_type) = body.rule_type {
        let valid_types = ["price_threshold", "cheapest_hours", "time_schedule", "manual"];
        if !valid_types.contains(&rule_type.as_str()) {
            return HttpResponse::BadRequest().body("Invalid rule_type");
        }
    }

    // Validate action if provided
    if let Some(ref action) = body.action {
        let valid_actions = ["turn_on", "turn_off", "toggle"];
        if !valid_actions.contains(&action.as_str()) {
            return HttpResponse::BadRequest().body("Invalid action");
        }
    }

    if let Some(ref config) = body.config {
        let rule_type = body.rule_type.as_deref().unwrap_or(&current_rule_type);
        if let Err(e) = validate_rule_config(rule_type, config) {
            return HttpResponse::BadRequest().body(e);
        }
    }

    // Build update query
    let now = crate::time::now_naive();

    // Update each field if provided
    if let Some(ref name) = body.name {
        diesel::update(automation_rules::table.filter(automation_rules::id.eq(rule_id)))
            .set(automation_rules::name.eq(name))
            .execute(&mut conn)
            .ok();
    }

    if let Some(ref rule_type) = body.rule_type {
        diesel::update(automation_rules::table.filter(automation_rules::id.eq(rule_id)))
            .set(automation_rules::rule_type.eq(rule_type))
            .execute(&mut conn)
            .ok();
    }

    if let Some(ref action) = body.action {
        diesel::update(automation_rules::table.filter(automation_rules::id.eq(rule_id)))
            .set(automation_rules::action.eq(action))
            .execute(&mut conn)
            .ok();
    }

    if let Some(ref config) = body.config {
        diesel::update(automation_rules::table.filter(automation_rules::id.eq(rule_id)))
            .set(automation_rules::config.eq(config))
            .execute(&mut conn)
            .ok();
    }

    if let Some(is_enabled) = body.is_enabled {
        diesel::update(automation_rules::table.filter(automation_rules::id.eq(rule_id)))
            .set(automation_rules::is_enabled.eq(is_enabled))
            .execute(&mut conn)
            .ok();
    }

    if let Some(priority) = body.priority {
        diesel::update(automation_rules::table.filter(automation_rules::id.eq(rule_id)))
            .set(automation_rules::priority.eq(priority))
            .execute(&mut conn)
            .ok();
    }

    // Update the updated_at timestamp
    diesel::update(automation_rules::table.filter(automation_rules::id.eq(rule_id)))
        .set(automation_rules::updated_at.eq(now))
        .execute(&mut conn)
        .ok();

    // Recompute schedules if config, enabled status, or rule type changed
    let should_recompute = body.config.is_some()
        || body.is_enabled.is_some()
        || body.rule_type.is_some()
        || body.action.is_some();

    if should_recompute {
        let schedule_service = ScheduleComputationService::new(pool.get_ref().clone());
        if let Err(e) = schedule_service.refresh_rule(rule_id) {
            log::warn!("Failed to recompute schedule for rule {}: {}", rule_id, e);
        }
    }

    if body.is_enabled == Some(false) {
        release_device(&pool, &registry, rule_id).await;
    }

    // Return updated rule
    match automation_rules::table
        .find(rule_id)
        .first::<AutomationRule>(&mut conn)
    {
        Ok(rule) => HttpResponse::Ok().json(rule),
        Err(_) => HttpResponse::InternalServerError().body("Error fetching updated rule"),
    }
}

/// Delete a rule
#[delete("/{rule_id}")]
pub async fn delete_rule(
    pool: web::Data<DbPool>,
    registry: web::Data<ProviderRegistry>,
    claims: Claims,
    path: web::Path<i32>,
) -> impl Responder {
    let rule_id = path.into_inner();
    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return HttpResponse::InternalServerError().body("Database connection error"),
    };

    let user_id: i32 = match claims.sub.parse() {
        Ok(id) => id,
        Err(_) => return HttpResponse::BadRequest().body("Invalid user ID"),
    };

    // Verify rule exists and belongs to user
    let rule_exists = automation_rules::table
        .filter(automation_rules::id.eq(rule_id))
        .filter(automation_rules::user_id.eq(user_id))
        .select(automation_rules::id)
        .first::<i32>(&mut conn)
        .is_ok();

    if !rule_exists {
        return HttpResponse::NotFound().body("Rule not found");
    }

    // Turn the device off first if this rule has it on right now
    // (the execution log cascades away with the rule)
    release_device(&pool, &registry, rule_id).await;

    // Delete rule (executions will cascade)
    match diesel::delete(automation_rules::table.filter(automation_rules::id.eq(rule_id)))
        .execute(&mut conn)
    {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({"deleted": true})),
        Err(_) => HttpResponse::InternalServerError().body("Failed to delete rule"),
    }
}

/// Toggle a rule's enabled status
#[post("/{rule_id}/toggle")]
pub async fn toggle_rule(
    pool: web::Data<DbPool>,
    registry: web::Data<ProviderRegistry>,
    claims: Claims,
    path: web::Path<i32>,
) -> impl Responder {
    let rule_id = path.into_inner();
    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return HttpResponse::InternalServerError().body("Database connection error"),
    };

    let user_id: i32 = match claims.sub.parse() {
        Ok(id) => id,
        Err(_) => return HttpResponse::BadRequest().body("Invalid user ID"),
    };

    // Get current rule
    let rule: AutomationRule = match automation_rules::table
        .filter(automation_rules::id.eq(rule_id))
        .filter(automation_rules::user_id.eq(user_id))
        .first(&mut conn)
    {
        Ok(r) => r,
        Err(_) => return HttpResponse::NotFound().body("Rule not found"),
    };

    // Toggle enabled status
    let new_status = !rule.is_enabled;
    let now = crate::time::now_naive();

    diesel::update(automation_rules::table.filter(automation_rules::id.eq(rule_id)))
        .set((
            automation_rules::is_enabled.eq(new_status),
            automation_rules::updated_at.eq(now),
        ))
        .execute(&mut conn)
        .ok();

    // Recompute schedules: enabled rules get their hours, disabled ones lose the pending ones
    let schedule_service = ScheduleComputationService::new(pool.get_ref().clone());
    if let Err(e) = schedule_service.refresh_rule(rule_id) {
        log::warn!("Failed to recompute schedule for toggled rule {}: {}", rule_id, e);
    }

    if !new_status {
        release_device(&pool, &registry, rule_id).await;
    }

    HttpResponse::Ok().json(serde_json::json!({
        "id": rule_id,
        "is_enabled": new_status
    }))
}

/// Get execution history for a rule
#[get("/{rule_id}/executions")]
pub async fn get_rule_executions(
    pool: web::Data<DbPool>,
    claims: Claims,
    path: web::Path<i32>,
    query: web::Query<PaginationQuery>,
) -> impl Responder {
    let rule_id = path.into_inner();
    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return HttpResponse::InternalServerError().body("Database connection error"),
    };

    let user_id: i32 = match claims.sub.parse() {
        Ok(id) => id,
        Err(_) => return HttpResponse::BadRequest().body("Invalid user ID"),
    };

    // Verify rule belongs to user
    let rule_exists = automation_rules::table
        .filter(automation_rules::id.eq(rule_id))
        .filter(automation_rules::user_id.eq(user_id))
        .select(automation_rules::id)
        .first::<i32>(&mut conn)
        .is_ok();

    if !rule_exists {
        return HttpResponse::NotFound().body("Rule not found");
    }

    let limit = query.limit.unwrap_or(50).min(100);
    let offset = query.offset.unwrap_or(0);

    let executions: Vec<RuleExecution> = match rule_executions::table
        .filter(rule_executions::rule_id.eq(rule_id))
        .order(rule_executions::executed_at.desc())
        .limit(limit as i64)
        .offset(offset as i64)
        .load(&mut conn)
    {
        Ok(e) => e,
        Err(_) => return HttpResponse::InternalServerError().body("Error fetching executions"),
    };

    let response: Vec<ExecutionResponse> = executions
        .into_iter()
        .map(|e| ExecutionResponse {
            id: e.id,
            rule_id: e.rule_id,
            executed_at: e.executed_at.to_string(),
            action_taken: e.action_taken,
            success: e.success,
            error_message: e.error_message,
            price_at_execution: e.price_at_execution,
        })
        .collect();

    HttpResponse::Ok().json(response)
}

#[derive(Deserialize)]
pub struct PaginationQuery {
    pub limit: Option<i32>,
    pub offset: Option<i32>,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_rule_request_deserialization() {
        let json = r#"{
            "device_id": 1,
            "name": "Night heating",
            "rule_type": "cheapest_hours",
            "action": "turn_on",
            "config": {"hours_needed": 3, "window_start": "00:00", "window_end": "08:00"}
        }"#;
        let request: CreateRuleRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.device_id, 1);
        assert_eq!(request.name, "Night heating");
        assert_eq!(request.rule_type, "cheapest_hours");
    }

    #[test]
    fn test_update_rule_request_partial() {
        let json = r#"{"is_enabled": false}"#;
        let request: UpdateRuleRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.is_enabled, Some(false));
        assert!(request.name.is_none());
    }

    #[test]
    fn test_pagination_query_defaults() {
        let json = r#"{}"#;
        let query: PaginationQuery = serde_json::from_str(json).unwrap();
        assert!(query.limit.is_none());
        assert!(query.offset.is_none());
    }

    #[test]
    fn test_validate_rule_config() {
        let ok = |t: &str, c: JsonValue| validate_rule_config(t, &c).is_ok();

        assert!(ok("cheapest_hours", serde_json::json!({"cheapest_hours": 6})));
        assert!(ok(
            "cheapest_hours",
            serde_json::json!({"cheapest_hours": 6, "time_range_start": "19:00", "time_range_end": "07:00"})
        ));
        assert!(!ok("cheapest_hours", serde_json::json!({"cheapest_hours": 0})));
        assert!(!ok("cheapest_hours", serde_json::json!({"cheapest_hours": "6"})));
        assert!(!ok("cheapest_hours", serde_json::json!({"cheapest_hours": 6, "time_range_start": "19:00"})));
        assert!(!ok("time_schedule", serde_json::json!({"time_range_start": "7h30"})));
        assert!(ok("price_threshold", serde_json::json!({"price_threshold": 0.12})));
        assert!(!ok("price_threshold", serde_json::json!({"price_threshold": "0,12"})));
        assert!(ok("manual", serde_json::json!({})));

        let windows = |w: JsonValue, distribution: &str| {
            serde_json::json!({"cheapest_hours": 3, "windows": w, "distribution": distribution})
        };
        assert!(ok("cheapest_hours", windows(serde_json::json!([
            {"start": "14:00", "end": "16:00", "hours": 1},
            {"start": "20:00", "end": "06:00", "hours": 4}
        ]), "per_window")));
        // Overlap (15:00 in both)
        assert!(!ok("cheapest_hours", windows(serde_json::json!([
            {"start": "14:00", "end": "16:00"}, {"start": "15:00", "end": "18:00"}
        ]), "total")));
        // Overlap across midnight
        assert!(!ok("cheapest_hours", windows(serde_json::json!([
            {"start": "22:00", "end": "02:00"}, {"start": "01:00", "end": "03:00"}
        ]), "total")));
        // 3 h do not fit in a 2 h window
        assert!(!ok("cheapest_hours", windows(serde_json::json!([
            {"start": "14:00", "end": "16:00", "hours": 3}
        ]), "per_window")));
        assert!(!ok("cheapest_hours", windows(serde_json::json!([{"start": "14h", "end": "16:00"}]), "total")));
        assert!(!ok("cheapest_hours", windows(serde_json::json!([]), "random")));
        assert!(!ok("manual", serde_json::json!([])));
    }
}
