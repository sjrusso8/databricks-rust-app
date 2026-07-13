# Rust App on Databricks

A minimal REST API — **axum** + **sqlx** — backed by **Databricks Lakebase**
(autoscaling Postgres), deployable as a Databricks App via an asset bundle.

## Endpoints

| Method | Path              | Description                     |
| ------ | ----------------- | ------------------------------- |
| GET    | `/health`         | Liveness probe                  |
| GET    | `/api/users`      | List all users                  |
| GET    | `/api/users/{id}` | Get a single user (404 if none) |

## Layout

```
src/main.rs                      the entire app (~140 lines)
migrations/0001_create_users.sql users table + 10 seed rows
main.py                          launcher: mints the Lakebase token, execs the binary
app.yaml                         Databricks App spec
databricks.yml                   bundle: Lakebase project + app (bound together)
```

The app connects using standard `PG*` env vars (or a single `DATABASE_URL`) and
keeps its objects in a `rust_api` schema it owns.

## Local dev

```bash
cp .env.example .env    # set PGHOST + PGUSER (PGHOST from the command below)

# host of the deployed Lakebase endpoint:
databricks postgres list-endpoints projects/rust-api-app/branches/production \
  -p fevm -o json | jq -r '.[0].status.hosts.host'

# export a fresh OAuth token as the password (valid ~1h):
export PGPASSWORD=$(databricks postgres generate-database-credential \
  projects/rust-api-app/branches/production/endpoints/primary -p fevm -o json | jq -r .token)

cargo run
curl localhost:8080/api/users
```

On startup the app creates the `rust_api` schema, applies the migration
(seeding `users`), and serves.

## Deploy (Lakebase + app, together)

`databricks.yml` defines both resources; the app is bound to the Lakebase
project with `CAN_CONNECT_AND_CREATE`, so the App runtime injects
`PGHOST/PGUSER/PGDATABASE/PGSSLMODE` plus the service-principal OAuth
credentials. `main.py` mints the password from those and hands off to the binary.

```bash
# Build a static Linux binary (see docs/deploying-rust-on-databricks-apps.md)
rustup target add x86_64-unknown-linux-musl
cargo build --release --target x86_64-unknown-linux-musl

databricks bundle deploy -p fevm
databricks bundle run rust_api -p fevm    # start (or restart) the app
```

> **Token lifetime:** `main.py` mints the DB token once at startup (~1h). Fine
> for a POC; a long-running app should refresh it and recycle the pool.
