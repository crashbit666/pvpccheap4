-- Whether the plug reports its power: NULL until asked
ALTER TABLE devices ADD COLUMN measures_power BOOLEAN;

-- Power measured every few minutes while a plug is on, for the real savings
CREATE TABLE power_readings (
    id SERIAL PRIMARY KEY,
    device_id INTEGER NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    measured_at TIMESTAMP NOT NULL,
    watts REAL NOT NULL
);
CREATE INDEX power_readings_device_time ON power_readings (device_id, measured_at);
