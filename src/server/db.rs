use dioxus::fullstack::Lazy;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use std::str::FromStr;

pub static DB: Lazy<SqlitePool> = Lazy::new(|| async move {
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(
            SqliteConnectOptions::from_str("sqlite:photovault.db")
                .expect("Invalid database URL")
                .create_if_missing(true),
        )
        .await?;

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    tracing::info!("Database connected and migrations applied");

    dioxus::Ok(pool)
});
