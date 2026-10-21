-- Power the appliance draws when on, to turn cheap hours into euros saved
ALTER TABLE devices ADD COLUMN power_watts INTEGER;
