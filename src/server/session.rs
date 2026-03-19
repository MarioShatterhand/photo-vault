use crate::server::db::DB;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Json};
use axum::{extract::Request, middleware::Next, response::Response};
use rand::RngCore;
use sha2::{Digest, Sha256};

const COOKIE_NAME: &str = "photovault_session";
const SESSION_DURATION_DAYS: i64 = 30;

/// Generate a random 32-byte hex token
fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

/// Hash a token with SHA-256
fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

/// Create a new session in the database, returns the raw token
pub async fn create_session(user_id: i64) -> Result<String, sqlx::Error> {
    let token = generate_token();
    let token_hash = hash_token(&token);
    let now = chrono::Utc::now();
    let expires_at = now + chrono::Duration::days(SESSION_DURATION_DAYS);

    sqlx::query(
        "INSERT INTO sessions (user_id, token_hash, created_at, last_used, expires_at) VALUES (?, ?, ?, ?, ?)"
    )
    .bind(user_id)
    .bind(&token_hash)
    .bind(now.to_rfc3339())
    .bind(now.to_rfc3339())
    .bind(expires_at.to_rfc3339())
    .execute(&*DB)
    .await?;

    Ok(token)
}

/// Validate a session token, returns user_id if valid
pub async fn validate_session(token: &str) -> Result<Option<i64>, sqlx::Error> {
    let token_hash = hash_token(token);
    let now = chrono::Utc::now().to_rfc3339();

    let row: Option<(i64,)> = sqlx::query_as(
        "SELECT user_id FROM sessions WHERE token_hash = ? AND expires_at > ?"
    )
    .bind(&token_hash)
    .bind(&now)
    .fetch_optional(&*DB)
    .await?;

    if let Some((user_id,)) = row {
        // Update last_used
        sqlx::query("UPDATE sessions SET last_used = ? WHERE token_hash = ?")
            .bind(&now)
            .bind(&token_hash)
            .execute(&*DB)
            .await?;
        Ok(Some(user_id))
    } else {
        Ok(None)
    }
}

/// Destroy a session by raw token
pub async fn destroy_session(token: &str) -> Result<(), sqlx::Error> {
    let token_hash = hash_token(token);
    sqlx::query("DELETE FROM sessions WHERE token_hash = ?")
        .bind(&token_hash)
        .execute(&*DB)
        .await?;
    Ok(())
}

/// Build Set-Cookie header value for a session
pub fn create_session_cookie(token: &str) -> String {
    let max_age = SESSION_DURATION_DAYS * 24 * 60 * 60;
    format!(
        "{COOKIE_NAME}={token}; HttpOnly; SameSite=Strict; Path=/; Max-Age={max_age}"
    )
}

/// Build Set-Cookie header to clear the session cookie
pub fn clear_session_cookie() -> String {
    format!("{COOKIE_NAME}=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0")
}

/// Extract session token from Cookie header
pub fn extract_session_token(headers: &HeaderMap) -> Option<String> {
    let cookie_header = headers.get("cookie")?.to_str().ok()?;
    for part in cookie_header.split(';') {
        let part = part.trim();
        if let Some(value) = part.strip_prefix(&format!("{COOKIE_NAME}=")) {
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

/// Axum middleware that validates session and injects user_id into extensions
pub async fn auth_middleware(mut request: Request, next: Next) -> Result<Response, StatusCode> {
    let token = extract_session_token(request.headers())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let user_id = validate_session(&token)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;

    request.extensions_mut().insert(user_id);
    Ok(next.run(request).await)
}

/// GET /api/auth/status — check if app is set up and if user is authenticated
pub async fn auth_status(headers: HeaderMap) -> impl IntoResponse {
    let user_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(&*DB)
        .await
        .unwrap_or((0,));

    let setup = user_count.0 > 0;

    let mut authenticated = false;
    let mut user_id = None;

    if let Some(token) = extract_session_token(&headers) {
        if let Ok(Some(uid)) = validate_session(&token).await {
            authenticated = true;
            user_id = Some(uid);
        }
    }

    Json(serde_json::json!({
        "setup": setup,
        "authenticated": authenticated,
        "user_id": user_id
    }))
}
