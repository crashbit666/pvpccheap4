// @generated automatically by Diesel CLI.

diesel::table! {
    devices (id) {
        id -> Int4,
        integration_id -> Int4,
        external_id -> Text,
        name -> Text,
        device_type -> Text,
        is_managed -> Bool,
    }
}

diesel::table! {
    prices (timestamp) {
        timestamp -> Timestamp,
        price -> Float8,
        source -> Text,
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
    }
}

diesel::table! {
    users (id) {
        id -> Int4,
        username -> Text,
        password_hash -> Text,
        created_at -> Timestamp,
    }
}

diesel::joinable!(devices -> user_integrations (integration_id));
diesel::joinable!(schedules -> devices (device_id));
diesel::joinable!(schedules -> users (user_id));
diesel::joinable!(user_integrations -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(devices, prices, schedules, user_integrations, users,);
