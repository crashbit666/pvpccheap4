-- One price per zone and hour (Península, Balears, Ceuta, Melilla)
ALTER TABLE prices ADD COLUMN zone TEXT NOT NULL DEFAULT 'peninsula';
ALTER TABLE prices DROP CONSTRAINT prices_pkey;
ALTER TABLE prices ADD PRIMARY KEY (zone, timestamp);

-- The zone whose prices the user's rules follow
ALTER TABLE users ADD COLUMN zone TEXT NOT NULL DEFAULT 'peninsula';
