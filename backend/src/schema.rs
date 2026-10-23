// @generated automatically by Diesel CLI.
// Manual additions for new tables

diesel::table! {
    devices (id) {
        id -> Int4,
        integration_id -> Int4,
        external_id -> Text,
        name -> Text,
        device_type -> Text,
        is_managed -> Bool,
        is_on -> Bool,
        power_watts -> Nullable<Int4>,
        measures_power -> Nullable<Bool>,
    }
}

diesel::table! {
    power_readings (id) {
        id -> Int4,
        device_id -> Int4,
        measured_at -> Timestamp,
        watts -> Float4,
    }
}

diesel::table! {
    prices (series, timestamp) {
        timestamp -> Timestamp,
        price -> Float8,
        source -> Text,
        series -> Text,
        minutes -> Int2,
    }
}

diesel::table! {
    schedules (id) {
        id -> Int4,
        device_id -> Int4,
        user_id -> Int4,
        duration_minutes -> Int4,
        window_start -> Time,
        window_end -> Time,
        created_at -> Timestamp,
    }
}

diesel::table! {
    user_integrations (id) {
        id -> Int4,
        user_id -> Int4,
        provider_name -> Text,
        credentials_json -> Text,
        is_active -> Bool,
        created_at -> Timestamp,
        session_data -> Nullable<Jsonb>,
        session_expires_at -> Nullable<Timestamp>,
    }
}

diesel::table! {
    users (id) {
        id -> Int4,
        username -> Text,
        password_hash -> Text,
        created_at -> Timestamp,
        token_version -> Int4,
        is_premium -> Bool,
        email -> Nullable<Text>,
        email_verified -> Bool,
        zone -> Text,
        premium_until -> Nullable<Timestamp>,
        tariff -> Text,
        tariff_margin -> Float8,
    }
}

diesel::table! {
    subscriptions (id) {
        id -> Int4,
        user_id -> Int4,
        platform -> Text,
        product_id -> Text,
        purchase_token -> Text,
        state -> Text,
        expires_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    user_tokens (id) {
        id -> Int4,
        user_id -> Int4,
        kind -> Text,
        token_hash -> Text,
        email -> Text,
        created_at -> Timestamp,
        expires_at -> Timestamp,
        used_at -> Nullable<Timestamp>,
    }
}

diesel::table! {
    automation_rules (id) {
        id -> Int4,
        user_id -> Int4,
        device_id -> Int4,
        name -> Text,
        rule_type -> Text,
        action -> Text,
        config -> Jsonb,
        is_enabled -> Bool,
        priority -> Int4,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        last_triggered_at -> Nullable<Timestamp>,
    }
}

diesel::table! {
    rule_executions (id) {
        id -> Int4,
        rule_id -> Int4,
        executed_at -> Timestamp,
        action_taken -> Text,
        success -> Bool,
        error_message -> Nullable<Text>,
        price_at_execution -> Nullable<Float8>,
        device_state_before -> Nullable<Jsonb>,
        device_state_after -> Nullable<Jsonb>,
    }
}

diesel::table! {
    scheduled_executions (id) {
        id -> Int4,
        rule_id -> Int4,
        scheduled_hour -> Timestamp,
        expected_action -> Text,
        status -> Text,
        executed_at -> Nullable<Timestamp>,
        execution_id -> Nullable<Int4>,
        retry_count -> Int4,
        last_retry_at -> Nullable<Timestamp>,
        next_retry_at -> Nullable<Timestamp>,
        created_at -> Timestamp,
    }
}

diesel::joinable!(devices -> user_integrations (integration_id));
diesel::joinable!(power_readings -> devices (device_id));
diesel::joinable!(schedules -> devices (device_id));
diesel::joinable!(schedules -> users (user_id));
diesel::joinable!(user_integrations -> users (user_id));
diesel::joinable!(user_tokens -> users (user_id));
diesel::joinable!(subscriptions -> users (user_id));
diesel::joinable!(automation_rules -> users (user_id));
diesel::joinable!(automation_rules -> devices (device_id));
diesel::joinable!(rule_executions -> automation_rules (rule_id));
diesel::joinable!(scheduled_executions -> automation_rules (rule_id));

diesel::allow_tables_to_appear_in_same_query!(
    automation_rules,
    devices,
    power_readings,
    prices,
    rule_executions,
    scheduled_executions,
    schedules,
    subscriptions,
    user_integrations,
    user_tokens,
    users,
);
