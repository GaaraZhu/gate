-- Demo database for redact end-to-end testing.
-- All data is synthetic — emails and card numbers are fake.

CREATE TABLE users (
    id         SERIAL PRIMARY KEY,
    username   TEXT NOT NULL,
    plan       TEXT NOT NULL DEFAULT 'free',
    region     TEXT NOT NULL,
    status     TEXT NOT NULL DEFAULT 'active',
    email      TEXT NOT NULL,        -- PII
    credit_card TEXT NOT NULL        -- PII
);

-- Credit cards are well-known Luhn-valid test vectors.
INSERT INTO users (username, plan, region, status, email, credit_card) VALUES
  ('alice', 'pro',  'us-west',    'active', 'alice.johnson@example.com', '4111111111111111'),
  ('bob',   'free', 'eu-central', 'active', 'bob.williams@example.com',  '4012888888881881');
