use axum::extract::{Path, Query};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use axum::Extension;
use axum_extra::extract::Multipart;
use serde::Deserialize;
use image::imageops::FilterType;
use image::GenericImageView;
use sha2::{Digest, Sha256};
use crate::models::Photo;
use crate::server::db::DB;

/// Per-user storage layout:
///   uploads/{user_id}/{hash}.{ext}        — original
///   uploads/{user_id}/thumbs/{hash}.jpg   — thumbnail
fn user_thumb_dir(user_id: i64) -> String {
    format!("uploads/{user_id}/thumbs")
}
fn user_original_path(user_id: i64, filename: &str) -> String {
    format!("uploads/{user_id}/{filename}")
}
fn user_thumb_path(user_id: i64, hash: &str) -> String {
    format!("uploads/{user_id}/thumbs/{hash}.jpg")
}

const MAX_FILE_SIZE: usize = 20 * 1024 * 1024; // 20MB
const ALLOWED_TYPES: &[&str] = &["image/jpeg", "image/png", "image/webp", "image/gif"];

fn extension_from_content_type(content_type: &str) -> Option<&'static str> {
    match content_type {
        "image/jpeg" => Some("jpg"),
        "image/png" => Some("png"),
        "image/webp" => Some("webp"),
        "image/gif" => Some("gif"),
        _ => None,
    }
}

fn content_type_from_extension(ext: &str) -> &'static str {
    match ext {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        _ => "application/octet-stream",
    }
}

/// POST /api/upload
pub async fn upload_photo(
    Extension(user_id): Extension<i64>,
    mut multipart: Multipart,
) -> Result<Json<Photo>, (StatusCode, String)> {
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Multipart error: {e}")))?
    {
        let name = field.name().unwrap_or("").to_string();
        if name != "file" {
            continue;
        }

        let content_type = field
            .content_type()
            .unwrap_or("application/octet-stream")
            .to_string();

        if !ALLOWED_TYPES.contains(&content_type.as_str()) {
            return Err((
                StatusCode::BAD_REQUEST,
                format!("Unsupported file type: {content_type}. Allowed: JPEG, PNG, WebP, GIF"),
            ));
        }

        let original_name = field
            .file_name()
            .unwrap_or("unnamed")
            .to_string();

        let ext = extension_from_content_type(&content_type)
            .ok_or((StatusCode::BAD_REQUEST, "Unknown file extension".to_string()))?;

        let bytes = field
            .bytes()
            .await
            .map_err(|e| (StatusCode::BAD_REQUEST, format!("Failed to read file: {e}")))?;

        if bytes.len() > MAX_FILE_SIZE {
            return Err((
                StatusCode::PAYLOAD_TOO_LARGE,
                format!("File too large. Max size: 20MB, got: {}MB", bytes.len() / 1024 / 1024),
            ));
        }

        // Compute SHA-256 hash
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let hash = hex::encode(hasher.finalize());

        let filename = format!("{}.{}", hash, ext);

        // Per-user dedup: same user uploading same file = no-op, return existing.
        // Different users uploading the same file get independent rows + files.
        let existing: Option<Photo> = sqlx::query_as(
            "SELECT * FROM photos WHERE user_id = ? AND hash = ?"
        )
        .bind(user_id)
        .bind(&hash)
        .fetch_optional(&*DB)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

        if let Some(photo) = existing {
            return Ok(Json(photo));
        }

        // Lazily create per-user storage dirs (cheap if they already exist).
        tokio::fs::create_dir_all(user_thumb_dir(user_id))
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to create user dirs: {e}")))?;

        // Save original
        let original_path = user_original_path(user_id, &filename);
        tokio::fs::write(&original_path, &bytes)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to save file: {e}")))?;

        // Read dimensions and generate thumbnail
        let (width, height, thumb_bytes) = {
            let bytes = bytes.clone();
            tokio::task::spawn_blocking(move || -> Result<(u32, u32, Vec<u8>), String> {
                let img = image::load_from_memory(&bytes)
                    .map_err(|e| format!("Failed to decode image: {e}"))?;
                let (w, h) = img.dimensions();
                let thumb = img.resize(300, u32::MAX, FilterType::Lanczos3);
                let mut cursor = std::io::Cursor::new(Vec::new());
                thumb
                    .write_to(&mut cursor, image::ImageFormat::Jpeg)
                    .map_err(|e| format!("Failed to encode thumbnail: {e}"))?;
                Ok((w, h, cursor.into_inner()))
            })
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Task join error: {e}")))?
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?
        };

        // Save thumbnail (always JPEG)
        let thumb_path = user_thumb_path(user_id, &hash);
        tokio::fs::write(&thumb_path, &thumb_bytes)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to save thumbnail: {e}")))?;

        // Insert into database
        let size = bytes.len() as i64;
        let public_id = uuid::Uuid::new_v4().to_string();
        let photo: Photo = sqlx::query_as(
            "INSERT INTO photos (user_id, public_id, filename, original_name, hash, size, width, height) VALUES (?, ?, ?, ?, ?, ?, ?, ?) RETURNING *"
        )
        .bind(user_id)
        .bind(&public_id)
        .bind(&filename)
        .bind(&original_name)
        .bind(&hash)
        .bind(size)
        .bind(width as i64)
        .bind(height as i64)
        .fetch_one(&*DB)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB insert error: {e}")))?;

        tracing::info!("Uploaded photo: {} ({}) for user {}", photo.original_name, photo.filename, user_id);
        return Ok(Json(photo));
    }

    Err((StatusCode::BAD_REQUEST, "No file field found in upload".to_string()))
}

/// GET /api/photos/:public_id/thumb
pub async fn serve_thumbnail(
    Extension(user_id): Extension<i64>,
    Path(public_id): Path<String>,
) -> Result<Response, StatusCode> {
    let photo: Photo = sqlx::query_as(
        "SELECT * FROM photos WHERE public_id = ? AND user_id = ?",
    )
    .bind(&public_id)
    .bind(user_id)
    .fetch_optional(&*DB)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    let bytes = tokio::fs::read(user_thumb_path(user_id, &photo.hash))
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    Ok((
        [
            (header::CONTENT_TYPE, "image/jpeg"),
            (header::CACHE_CONTROL, "private, no-store"),
        ],
        bytes,
    ).into_response())
}

/// GET /api/photos/:public_id/full
pub async fn serve_full(
    Extension(user_id): Extension<i64>,
    Path(public_id): Path<String>,
) -> Result<Response, StatusCode> {
    let photo: Photo = sqlx::query_as(
        "SELECT * FROM photos WHERE public_id = ? AND user_id = ?",
    )
    .bind(&public_id)
    .bind(user_id)
    .fetch_optional(&*DB)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    let bytes = tokio::fs::read(user_original_path(user_id, &photo.filename))
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    let ext = photo.filename.rsplit('.').next().unwrap_or("jpg");
    let content_type = content_type_from_extension(ext);

    Ok((
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, "private, no-store"),
        ],
        bytes,
    ).into_response())
}

/// DELETE /api/photos/:public_id
pub async fn delete_photo(
    Extension(user_id): Extension<i64>,
    Path(public_id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let photo: Photo = sqlx::query_as(
        "SELECT * FROM photos WHERE public_id = ? AND user_id = ?",
    )
    .bind(&public_id)
    .bind(user_id)
    .fetch_optional(&*DB)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?
    .ok_or((StatusCode::NOT_FOUND, "Photo not found".to_string()))?;

    sqlx::query("DELETE FROM photos WHERE id = ?")
        .bind(photo.id)
        .execute(&*DB)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB delete error: {e}")))?;

    let original_path = user_original_path(user_id, &photo.filename);
    if let Err(e) = tokio::fs::remove_file(&original_path).await {
        tracing::warn!("Failed to remove file {}: {}", original_path, e);
    }

    let thumb_path = user_thumb_path(user_id, &photo.hash);
    if let Err(e) = tokio::fs::remove_file(&thumb_path).await {
        tracing::warn!("Failed to remove thumbnail {}: {}", thumb_path, e);
    }

    tracing::info!("Deleted photo: {} ({}) for user {}", photo.original_name, photo.public_id, user_id);
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct SearchParams {
    pub q: Option<String>,
}

/// GET /api/photos?q=search_term
pub async fn list_photos(
    Extension(user_id): Extension<i64>,
    Query(params): Query<SearchParams>,
) -> Result<Json<Vec<Photo>>, (StatusCode, String)> {
    let query = params.q.unwrap_or_default().trim().to_string();

    if query.is_empty() {
        let photos = sqlx::query_as::<_, Photo>(
            "SELECT * FROM photos WHERE user_id = ? ORDER BY created_at DESC",
        )
        .bind(user_id)
        .fetch_all(&*DB)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;
        return Ok(Json(photos));
    }

    let sanitized = sanitize_fts_query(&query);
    let photos = sqlx::query_as::<_, Photo>(
        "SELECT p.* FROM photos p JOIN photos_fts ON photos_fts.rowid = p.id \
         WHERE photos_fts MATCH ? AND p.user_id = ? ORDER BY rank",
    )
    .bind(&sanitized)
    .bind(user_id)
    .fetch_all(&*DB)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;
    Ok(Json(photos))
}

fn sanitize_fts_query(input: &str) -> String {
    input.split_whitespace()
        .filter(|w| !w.is_empty())
        .map(|word| {
            let escaped = word.replace('"', "\"\"");
            format!("\"{}\"*", escaped)
        })
        .collect::<Vec<_>>()
        .join(" ")
}

