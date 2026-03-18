use axum::extract::Path;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use axum_extra::extract::Multipart;
use image::imageops::FilterType;
use sha2::{Digest, Sha256};
use crate::models::Photo;
use crate::server::db::DB;

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
pub async fn upload_photo(mut multipart: Multipart) -> Result<Json<Photo>, (StatusCode, String)> {
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
        let original_path = format!("uploads/{}", filename);

        // Check for duplicate
        let existing: Option<Photo> = sqlx::query_as(
            "SELECT * FROM photos WHERE hash = ?"
        )
        .bind(&hash)
        .fetch_optional(&*DB)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

        if let Some(photo) = existing {
            return Ok(Json(photo));
        }

        // Save original
        tokio::fs::write(&original_path, &bytes)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to save file: {e}")))?;

        // Generate thumbnail (300px wide, preserve aspect ratio)
        let thumb_bytes = {
            let bytes = bytes.clone();
            tokio::task::spawn_blocking(move || -> Result<Vec<u8>, String> {
                let img = image::load_from_memory(&bytes)
                    .map_err(|e| format!("Failed to decode image: {e}"))?;
                let thumb = img.resize(300, u32::MAX, FilterType::Lanczos3);
                let mut cursor = std::io::Cursor::new(Vec::new());
                thumb
                    .write_to(&mut cursor, image::ImageFormat::Jpeg)
                    .map_err(|e| format!("Failed to encode thumbnail: {e}"))?;
                Ok(cursor.into_inner())
            })
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Task join error: {e}")))?
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?
        };

        // Save thumbnail (always JPEG for thumbnails)
        let thumb_filename = format!("{}.jpg", hash);
        let thumb_path = format!("uploads/thumbs/{}", thumb_filename);
        tokio::fs::write(&thumb_path, &thumb_bytes)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to save thumbnail: {e}")))?;

        // Insert into database
        let size = bytes.len() as i64;
        let public_id = uuid::Uuid::new_v4().to_string();
        let photo: Photo = sqlx::query_as(
            "INSERT INTO photos (public_id, filename, original_name, hash, size) VALUES (?, ?, ?, ?, ?) RETURNING *"
        )
        .bind(&public_id)
        .bind(&filename)
        .bind(&original_name)
        .bind(&hash)
        .bind(size)
        .fetch_one(&*DB)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB insert error: {e}")))?;

        tracing::info!("Uploaded photo: {} ({})", photo.original_name, photo.filename);
        return Ok(Json(photo));
    }

    Err((StatusCode::BAD_REQUEST, "No file field found in upload".to_string()))
}

/// GET /api/photos/:public_id/thumb
pub async fn serve_thumbnail(Path(public_id): Path<String>) -> Result<Response, StatusCode> {
    let photo: Photo = sqlx::query_as("SELECT * FROM photos WHERE public_id = ?")
        .bind(&public_id)
        .fetch_optional(&*DB)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    // Thumbnails are always JPEG
    let thumb_filename = format!("{}.jpg", photo.hash);
    let thumb_path = format!("uploads/thumbs/{}", thumb_filename);

    let bytes = tokio::fs::read(&thumb_path)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    Ok((
        [(header::CONTENT_TYPE, "image/jpeg")],
        bytes,
    ).into_response())
}

/// GET /api/photos/:public_id/full
pub async fn serve_full(Path(public_id): Path<String>) -> Result<Response, StatusCode> {
    let photo: Photo = sqlx::query_as("SELECT * FROM photos WHERE public_id = ?")
        .bind(&public_id)
        .fetch_optional(&*DB)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let file_path = format!("uploads/{}", photo.filename);
    let bytes = tokio::fs::read(&file_path)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    let ext = photo.filename.rsplit('.').next().unwrap_or("jpg");
    let content_type = content_type_from_extension(ext);

    Ok((
        [(header::CONTENT_TYPE, content_type)],
        bytes,
    ).into_response())
}

