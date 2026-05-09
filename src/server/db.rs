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

    // Per-user subdirectories (uploads/{user_id}/ and uploads/{user_id}/thumbs/)
    // are created on demand by upload_photo.
    tokio::fs::create_dir_all("uploads")
        .await
        .expect("Failed to create uploads directory");

    if let Err(e) = relocate_legacy_uploads(&pool).await {
        tracing::warn!("Legacy upload relocation failed: {e}");
    }

    dioxus::Ok(pool)
});

/// One-time, idempotent migration of pre-multi-user file layout:
///   uploads/{filename}            → uploads/{user_id}/{filename}
///   uploads/thumbs/{hash}.jpg     → uploads/{user_id}/thumbs/{hash}.jpg
///
/// On a fresh install (no legacy files at top-level uploads/), this is a no-op.
/// On subsequent runs after a successful relocation, also a no-op (the files
/// are no longer at the top level).
async fn relocate_legacy_uploads(pool: &SqlitePool) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    relocate_originals(pool).await?;
    relocate_thumbs(pool).await?;
    Ok(())
}

async fn relocate_originals(pool: &SqlitePool) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut entries = tokio::fs::read_dir("uploads").await?;
    while let Some(entry) = entries.next_entry().await? {
        let meta = entry.metadata().await?;
        if !meta.is_file() {
            continue;
        }
        let filename = match entry.file_name().into_string() {
            Ok(s) => s,
            Err(_) => continue,
        };

        let user_id: Option<(i64,)> = sqlx::query_as(
            "SELECT user_id FROM photos WHERE filename = ?",
        )
        .bind(&filename)
        .fetch_optional(pool)
        .await?;

        let Some((user_id,)) = user_id else {
            tracing::warn!("Legacy file uploads/{filename} has no matching DB row, skipping");
            continue;
        };

        let dest_dir = format!("uploads/{user_id}");
        tokio::fs::create_dir_all(&dest_dir).await?;
        let dest = format!("{dest_dir}/{filename}");
        tokio::fs::rename(entry.path(), &dest).await?;
        tracing::info!("Relocated uploads/{filename} -> {dest}");
    }
    Ok(())
}

async fn relocate_thumbs(pool: &SqlitePool) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let thumbs_root = std::path::Path::new("uploads/thumbs");
    if !thumbs_root.exists() {
        return Ok(());
    }
    let mut entries = tokio::fs::read_dir(thumbs_root).await?;
    while let Some(entry) = entries.next_entry().await? {
        let meta = entry.metadata().await?;
        if !meta.is_file() {
            continue;
        }
        let filename = match entry.file_name().into_string() {
            Ok(s) => s,
            Err(_) => continue,
        };
        // Thumbs are named {hash}.jpg
        let hash = match filename.strip_suffix(".jpg") {
            Some(h) => h,
            None => continue,
        };

        let user_id: Option<(i64,)> = sqlx::query_as(
            "SELECT user_id FROM photos WHERE hash = ?",
        )
        .bind(hash)
        .fetch_optional(pool)
        .await?;

        let Some((user_id,)) = user_id else {
            tracing::warn!("Legacy thumb {filename} has no matching DB row, skipping");
            continue;
        };

        let dest_dir = format!("uploads/{user_id}/thumbs");
        tokio::fs::create_dir_all(&dest_dir).await?;
        let dest = format!("{dest_dir}/{filename}");
        tokio::fs::rename(entry.path(), &dest).await?;
        tracing::info!("Relocated uploads/thumbs/{filename} -> {dest}");
    }

    // Remove the now-empty legacy thumbs dir if empty.
    if thumbs_root.exists() {
        if let Ok(mut leftover) = tokio::fs::read_dir(thumbs_root).await {
            if leftover.next_entry().await.ok().flatten().is_none() {
                let _ = tokio::fs::remove_dir(thumbs_root).await;
            }
        }
    }
    Ok(())
}
