ALTER TABLE users DROP COLUMN tariff_margin;
ALTER TABLE users DROP COLUMN tariff;
DELETE FROM prices WHERE series NOT IN ('peninsula', 'balears', 'ceuta', 'melilla');
ALTER TABLE prices DROP COLUMN minutes;
ALTER TABLE prices RENAME COLUMN series TO zone;
