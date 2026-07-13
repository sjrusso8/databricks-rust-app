-- Create the users table and seed 10 records.
-- Applied automatically at startup via sqlx::migrate! and tracked in the
-- _sqlx_migrations table, so re-running is a no-op.

CREATE TABLE IF NOT EXISTS users (
    id         BIGSERIAL PRIMARY KEY,
    name       TEXT        NOT NULL,
    email      TEXT        NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

INSERT INTO users (name, email) VALUES
    ('Ada Lovelace',        'ada.lovelace@example.com'),
    ('Alan Turing',         'alan.turing@example.com'),
    ('Grace Hopper',        'grace.hopper@example.com'),
    ('Katherine Johnson',   'katherine.johnson@example.com'),
    ('Linus Torvalds',      'linus.torvalds@example.com'),
    ('Margaret Hamilton',   'margaret.hamilton@example.com'),
    ('Dennis Ritchie',      'dennis.ritchie@example.com'),
    ('Barbara Liskov',      'barbara.liskov@example.com'),
    ('Ken Thompson',        'ken.thompson@example.com'),
    ('Radia Perlman',       'radia.perlman@example.com')
ON CONFLICT (email) DO NOTHING;
