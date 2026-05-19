ALTER TABLE threads ADD COLUMN service_tier TEXT;
ALTER TABLE threads ADD COLUMN service_tier_known INTEGER NOT NULL DEFAULT 0;
