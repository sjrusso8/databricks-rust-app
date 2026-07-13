//! rust-api — axum + sqlx REST service backed by Databricks Lakebase.
//!
//!   GET /health          liveness probe
//!   GET /api/users       list all users
//!   GET /api/users/{id}  get one user (404 if absent)
//!
//! Connection: `DATABASE_URL` if set, otherwise the standard `PG*` env vars
//! (`PGHOST`, `PGUSER`, `PGPASSWORD`, …). Databricks Apps inject those from the
//! bound Lakebase resource (main.py mints the password); locally you export
//! them yourself — see the README. Objects live in a schema we own, sidestepping
//! the restricted `public` schema of Lakebase's default database.

use anyhow::{Context, Result};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::json;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{Executor, PgPool};
use std::{env, time::Duration};

const SCHEMA: &str = "rust_api";

#[derive(Serialize, sqlx::FromRow)]
struct User {
    id: i64,
    name: String,
    email: String,
    created_at: DateTime<Utc>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();

    let opts = match env::var("DATABASE_URL") {
        Ok(url) => url.parse::<PgConnectOptions>().context("bad DATABASE_URL")?,
        Err(_) => PgConnectOptions::new(), // reads PGHOST/PGUSER/PGPASSWORD/PGSSLMODE/…
    };

    // Default every connection to our schema before touching any table.
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(10))
        .after_connect(|conn, _| {
            Box::pin(async move {
                conn.execute(format!("SET search_path TO {SCHEMA}, public").as_str())
                    .await?;
                Ok(())
            })
        })
        .connect_with(opts)
        .await
        .context("failed to connect to Lakebase")?;

    pool.execute(format!("CREATE SCHEMA IF NOT EXISTS {SCHEMA}").as_str())
        .await
        .context("failed to create schema")?;
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .context("failed to run migrations")?;

    let app = Router::new()
        .route("/health", get(|| async { Json(json!({ "status": "ok" })) }))
        .route("/api/users", get(list_users))
        .route("/api/users/{id}", get(get_user))
        .with_state(pool);

    let port = env::var("DATABRICKS_APP_PORT")
        .or_else(|_| env::var("PORT"))
        .unwrap_or_else(|_| "8080".into());
    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .with_context(|| format!("failed to bind {addr}"))?;
    println!("listening on {addr}");

    axum::serve(listener, app).await.context("server error")?;
    Ok(())
}

async fn list_users(State(pool): State<PgPool>) -> Result<Json<Vec<User>>, ApiError> {
    let users = sqlx::query_as::<_, User>("SELECT id, name, email, created_at FROM users ORDER BY id")
        .fetch_all(&pool)
        .await?;
    Ok(Json(users))
}

async fn get_user(
    State(pool): State<PgPool>,
    Path(id): Path<i64>,
) -> Result<Json<User>, ApiError> {
    sqlx::query_as::<_, User>("SELECT id, name, email, created_at FROM users WHERE id = $1")
        .bind(id)
        .fetch_optional(&pool)
        .await?
        .map(Json)
        .ok_or(ApiError::NotFound(id))
}

/// Maps failures onto HTTP responses: missing row → 404, anything else → 500.
enum ApiError {
    NotFound(i64),
    Db(sqlx::Error),
}

impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        ApiError::Db(e)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (code, msg) = match self {
            ApiError::NotFound(id) => (StatusCode::NOT_FOUND, format!("user {id} not found")),
            ApiError::Db(e) => {
                eprintln!("database error: {e}");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal server error".into())
            }
        };
        (code, Json(json!({ "error": msg }))).into_response()
    }
}
