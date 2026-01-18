-- Add is_on column to devices table to cache the last known state
ALTER TABLE devices ADD COLUMN is_on BOOLEAN NOT NULL DEFAULT FALSE;
