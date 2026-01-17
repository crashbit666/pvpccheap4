-- Clean up old tables
DROP TABLE IF EXISTS schedules CASCADE;
DROP TABLE IF EXISTS prices CASCADE;
DROP TABLE IF EXISTS devices CASCADE;
DROP TABLE IF EXISTS user_integrations CASCADE;
DROP TABLE IF EXISTS users CASCADE;

-- Users table
CREATE TABLE users (
    id SERIAL PRIMARY KEY,
    username TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

-- Store prices (global)
CREATE TABLE prices (
    timestamp TIMESTAMP PRIMARY KEY,
    price DOUBLE PRECISION NOT NULL,
    source TEXT NOT NULL
);

-- Store integrations (e.g. Meross credentials for a user)
CREATE TABLE user_integrations (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    provider_name TEXT NOT NULL, -- "meross", "tuya", "shelly"
    credentials_json TEXT NOT NULL, -- Encrypted JSON with login details
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

-- Store synced devices
CREATE TABLE devices (
    id SERIAL PRIMARY KEY,
    integration_id INTEGER NOT NULL REFERENCES user_integrations(id) ON DELETE CASCADE,
    external_id TEXT NOT NULL, -- ID in Meross cloud
    name TEXT NOT NULL,
    device_type TEXT NOT NULL, -- "switch", "light"
    is_managed BOOLEAN NOT NULL DEFAULT FALSE -- If TRUE, we control it automatically
);

-- Schedules linked to devices
CREATE TABLE schedules (
    id SERIAL PRIMARY KEY,
    device_id INTEGER NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    duration_minutes INTEGER NOT NULL,
    window_start TIME NOT NULL,
    window_end TIME NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);
