-- Prices hold several series now (PVPC by zone, day-ahead markets, tolls),
-- each at its own resolution: 60 or 15 minutes per row
ALTER TABLE prices RENAME COLUMN zone TO series;
ALTER TABLE prices ADD COLUMN minutes SMALLINT NOT NULL DEFAULT 60;

-- The user's tariff: PVPC, or indexed to the day-ahead market with a margin
ALTER TABLE users ADD COLUMN tariff TEXT NOT NULL DEFAULT 'pvpc';
ALTER TABLE users ADD COLUMN tariff_margin DOUBLE PRECISION NOT NULL DEFAULT 0;
