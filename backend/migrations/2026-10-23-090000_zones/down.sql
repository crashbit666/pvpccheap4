ALTER TABLE users DROP COLUMN zone;
DELETE FROM prices WHERE zone <> 'peninsula';
ALTER TABLE prices DROP CONSTRAINT prices_pkey;
ALTER TABLE prices ADD PRIMARY KEY (timestamp);
ALTER TABLE prices DROP COLUMN zone;
