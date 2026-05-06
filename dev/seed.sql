-- Demo database for gate end-to-end testing.
-- All data is synthetic — emails and card numbers are fake.

CREATE TABLE users (
    id               SERIAL PRIMARY KEY,
    full_name        TEXT NOT NULL,      -- PII
    email            TEXT NOT NULL,      -- PII
    status           TEXT NOT NULL DEFAULT 'active',
    created_at       TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_login_at    TIMESTAMP
);

-- Synthetic user data for gate end-to-end testing.
INSERT INTO users (full_name, email, status, created_at, last_login_at) VALUES
  ('Alice Johnson',   'alice.johnson@example.com',   'active',   '2023-01-15 10:30:00', '2024-05-06 14:22:00'),
  ('Bob Williams',    'bob.williams@example.com',    'active',   '2023-02-20 14:45:00', '2024-05-05 09:15:00'),
  ('Carol Martinez',  'carol.martinez@example.com',  'active',   '2023-03-10 09:15:00', '2024-05-04 16:30:00'),
  ('David Chen',      'david.chen@example.com',      'inactive', '2023-04-05 16:20:00', '2024-04-15 11:00:00'),
  ('Eve Okonkwo',     'eve.okonkwo@example.com',     'active',   '2023-05-12 11:00:00', '2024-05-06 13:45:00');
