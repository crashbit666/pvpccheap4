-- Paid until (store subscriptions); is_premium stays as a manual grant
ALTER TABLE users ADD COLUMN premium_until TIMESTAMP;

-- Store subscriptions, re-checked with the store before they expire
CREATE TABLE subscriptions (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    platform TEXT NOT NULL,
    product_id TEXT NOT NULL,
    purchase_token TEXT NOT NULL UNIQUE,
    state TEXT NOT NULL,
    expires_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL DEFAULT NOW()
);
CREATE INDEX subscriptions_user ON subscriptions (user_id);
