use crate::server::db::DB;
use crate::server::session::{
    clear_session_cookie, create_session, create_session_cookie, destroy_session,
    extract_session_token,
};
use axum::extract::Path;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::Json;
use axum::Extension;
use serde::Deserialize;
use std::sync::LazyLock;
use webauthn_rs::prelude::*;
use webauthn_rs::WebauthnBuilder;

static WEBAUTHN: LazyLock<Webauthn> = LazyLock::new(|| {
    let rp_id = "localhost";
    let rp_origin = url::Url::parse("http://localhost:8080").expect("Invalid URL");
    WebauthnBuilder::new(rp_id, &rp_origin)
        .expect("Failed to build Webauthn")
        .rp_name("PhotoVault")
        .build()
        .expect("Failed to build Webauthn")
});

#[derive(Deserialize)]
pub struct RegisterStartRequest {
    pub username: String,
}

/// POST /api/auth/register/start
pub async fn register_start(
    Json(body): Json<RegisterStartRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // First-register-wins: block if any user exists
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(&*DB)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    if count.0 > 0 {
        return Err((StatusCode::FORBIDDEN, "Registration closed".to_string()));
    }

    let user_uuid = uuid::Uuid::new_v4();

    let (ccr, reg_state) = WEBAUTHN
        .start_passkey_registration(user_uuid, &body.username, &body.username, None)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("WebAuthn error: {e}")))?;

    // Store challenge state
    let challenge_id = uuid::Uuid::new_v4().to_string();
    let state_json = serde_json::json!({
        "username": body.username,
        "uuid": user_uuid.to_string(),
        "state": serde_json::to_value(&reg_state).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Serialize error: {e}")))?
    });
    let now = chrono::Utc::now();
    let expires_at = now + chrono::Duration::minutes(5);

    sqlx::query(
        "INSERT INTO webauthn_challenges (challenge_id, challenge_type, state_json, created_at, expires_at) VALUES (?, 'registration', ?, ?, ?)"
    )
    .bind(&challenge_id)
    .bind(serde_json::to_string(&state_json).unwrap())
    .bind(now.to_rfc3339())
    .bind(expires_at.to_rfc3339())
    .execute(&*DB)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    Ok(Json(serde_json::json!({
        "challenge_id": challenge_id,
        "options": serde_json::to_value(&ccr).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Serialize error: {e}")))?
    })))
}

/// POST /api/auth/register/finish
pub async fn register_finish(
    Json(body): Json<serde_json::Value>,
) -> Result<(StatusCode, HeaderMap, Json<serde_json::Value>), (StatusCode, String)> {
    let challenge_id = body["challenge_id"]
        .as_str()
        .ok_or((StatusCode::BAD_REQUEST, "Missing challenge_id".to_string()))?;

    // Look up challenge
    let row: Option<(String, String)> = sqlx::query_as(
        "SELECT state_json, expires_at FROM webauthn_challenges WHERE challenge_id = ? AND challenge_type = 'registration'"
    )
    .bind(challenge_id)
    .fetch_optional(&*DB)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    let (state_json_str, expires_at) = row.ok_or((StatusCode::BAD_REQUEST, "Challenge not found".to_string()))?;

    // Check expiry
    let expires = chrono::DateTime::parse_from_rfc3339(&expires_at)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Date parse error: {e}")))?;
    if chrono::Utc::now() > expires {
        return Err((StatusCode::BAD_REQUEST, "Challenge expired".to_string()));
    }

    let state_wrapper: serde_json::Value = serde_json::from_str(&state_json_str)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("JSON parse error: {e}")))?;

    let username = state_wrapper["username"]
        .as_str()
        .ok_or((StatusCode::INTERNAL_SERVER_ERROR, "Missing username in state".to_string()))?
        .to_string();
    let user_uuid = state_wrapper["uuid"]
        .as_str()
        .ok_or((StatusCode::INTERNAL_SERVER_ERROR, "Missing uuid in state".to_string()))?
        .to_string();

    let reg_state: PasskeyRegistration = serde_json::from_value(state_wrapper["state"].clone())
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Deserialize state error: {e}")))?;

    // Parse the credential from browser
    let credential: RegisterPublicKeyCredential = serde_json::from_value(body["credential"].clone())
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid credential: {e}")))?;

    // Finish registration
    let passkey = WEBAUTHN
        .finish_passkey_registration(&credential, &reg_state)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Registration failed: {e}")))?;

    // Create user
    let user_id: (i64,) = sqlx::query_as(
        "INSERT INTO users (uuid, username) VALUES (?, ?) RETURNING id"
    )
    .bind(&user_uuid)
    .bind(&username)
    .fetch_one(&*DB)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    // Store credential
    let cred_id_b64 = base64url_encode(passkey.cred_id());
    let passkey_json = serde_json::to_string(&passkey)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Serialize error: {e}")))?;

    sqlx::query(
        "INSERT INTO credentials (user_id, credential_id, passkey_json) VALUES (?, ?, ?)"
    )
    .bind(user_id.0)
    .bind(&cred_id_b64)
    .bind(&passkey_json)
    .execute(&*DB)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    // Delete challenge
    sqlx::query("DELETE FROM webauthn_challenges WHERE challenge_id = ?")
        .bind(challenge_id)
        .execute(&*DB)
        .await
        .ok();

    // Create session
    let token = create_session(user_id.0)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Session error: {e}")))?;

    let mut headers = HeaderMap::new();
    headers.insert(header::SET_COOKIE, create_session_cookie(&token).parse().unwrap());

    Ok((StatusCode::OK, headers, Json(serde_json::json!({"status": "ok", "username": username}))))
}

/// POST /api/auth/login/start
pub async fn login_start() -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // Get all credentials
    let rows: Vec<(String,)> = sqlx::query_as("SELECT passkey_json FROM credentials")
        .fetch_all(&*DB)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    if rows.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "No registered credentials".to_string()));
    }

    let passkeys: Vec<Passkey> = rows
        .iter()
        .filter_map(|(json,)| serde_json::from_str(json).ok())
        .collect();

    let (rcr, auth_state) = WEBAUTHN
        .start_passkey_authentication(&passkeys)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("WebAuthn error: {e}")))?;

    let challenge_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now();
    let expires_at = now + chrono::Duration::minutes(5);

    let state_json = serde_json::to_string(&auth_state)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Serialize error: {e}")))?;

    sqlx::query(
        "INSERT INTO webauthn_challenges (challenge_id, challenge_type, state_json, created_at, expires_at) VALUES (?, 'authentication', ?, ?, ?)"
    )
    .bind(&challenge_id)
    .bind(&state_json)
    .bind(now.to_rfc3339())
    .bind(expires_at.to_rfc3339())
    .execute(&*DB)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    Ok(Json(serde_json::json!({
        "challenge_id": challenge_id,
        "options": serde_json::to_value(&rcr).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Serialize error: {e}")))?
    })))
}

/// POST /api/auth/login/finish
pub async fn login_finish(
    Json(body): Json<serde_json::Value>,
) -> Result<(StatusCode, HeaderMap, Json<serde_json::Value>), (StatusCode, String)> {
    let challenge_id = body["challenge_id"]
        .as_str()
        .ok_or((StatusCode::BAD_REQUEST, "Missing challenge_id".to_string()))?;

    let row: Option<(String, String)> = sqlx::query_as(
        "SELECT state_json, expires_at FROM webauthn_challenges WHERE challenge_id = ? AND challenge_type = 'authentication'"
    )
    .bind(challenge_id)
    .fetch_optional(&*DB)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    let (state_json_str, expires_at) = row.ok_or((StatusCode::BAD_REQUEST, "Challenge not found".to_string()))?;

    let expires = chrono::DateTime::parse_from_rfc3339(&expires_at)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Date parse error: {e}")))?;
    if chrono::Utc::now() > expires {
        return Err((StatusCode::BAD_REQUEST, "Challenge expired".to_string()));
    }

    let auth_state: PasskeyAuthentication = serde_json::from_str(&state_json_str)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Deserialize error: {e}")))?;

    let credential: PublicKeyCredential = serde_json::from_value(body["credential"].clone())
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid credential: {e}")))?;

    let auth_result = WEBAUTHN
        .finish_passkey_authentication(&credential, &auth_state)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Authentication failed: {e}")))?;

    // Find credential and user
    let cred_id_b64 = base64url_encode(auth_result.cred_id());
    let row: Option<(i64, i64)> = sqlx::query_as(
        "SELECT id, user_id FROM credentials WHERE credential_id = ?"
    )
    .bind(&cred_id_b64)
    .fetch_optional(&*DB)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    let (cred_db_id, user_id) = row.ok_or((StatusCode::INTERNAL_SERVER_ERROR, "Credential not found in DB".to_string()))?;

    // Update last_used and passkey state (counter)
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query("UPDATE credentials SET last_used = ? WHERE id = ?")
        .bind(&now)
        .bind(cred_db_id)
        .execute(&*DB)
        .await
        .ok();

    // Delete challenge
    sqlx::query("DELETE FROM webauthn_challenges WHERE challenge_id = ?")
        .bind(challenge_id)
        .execute(&*DB)
        .await
        .ok();

    let token = create_session(user_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Session error: {e}")))?;

    let mut headers = HeaderMap::new();
    headers.insert(header::SET_COOKIE, create_session_cookie(&token).parse().unwrap());

    Ok((StatusCode::OK, headers, Json(serde_json::json!({"status": "ok"}))))
}

/// POST /api/auth/logout
pub async fn logout(headers: HeaderMap) -> Result<(StatusCode, HeaderMap, Json<serde_json::Value>), (StatusCode, String)> {
    if let Some(token) = extract_session_token(&headers) {
        destroy_session(&token).await.ok();
    }

    let mut resp_headers = HeaderMap::new();
    resp_headers.insert(header::SET_COOKIE, clear_session_cookie().parse().unwrap());

    Ok((StatusCode::OK, resp_headers, Json(serde_json::json!({"status": "ok"}))))
}

/// GET /api/auth/passkeys
pub async fn list_passkeys(
    Extension(user_id): Extension<i64>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let rows: Vec<(i64, String, String, Option<String>)> = sqlx::query_as(
        "SELECT id, name, created_at, last_used FROM credentials WHERE user_id = ? ORDER BY created_at"
    )
    .bind(user_id)
    .fetch_all(&*DB)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    let passkeys: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|(id, name, created_at, last_used)| {
            serde_json::json!({
                "id": id,
                "name": name,
                "created_at": created_at,
                "last_used": last_used
            })
        })
        .collect();

    Ok(Json(serde_json::json!(passkeys)))
}

/// POST /api/auth/passkeys/add/start
pub async fn add_passkey_start(
    Extension(user_id): Extension<i64>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let user: (String, String) = sqlx::query_as(
        "SELECT uuid, username FROM users WHERE id = ?"
    )
    .bind(user_id)
    .fetch_one(&*DB)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    let user_uuid = uuid::Uuid::parse_str(&user.0)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("UUID parse error: {e}")))?;

    // Get existing credentials to exclude
    let existing_rows: Vec<(String,)> = sqlx::query_as(
        "SELECT passkey_json FROM credentials WHERE user_id = ?"
    )
    .bind(user_id)
    .fetch_all(&*DB)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    let existing: Vec<Passkey> = existing_rows
        .iter()
        .filter_map(|(json,)| serde_json::from_str(json).ok())
        .collect();

    let exclude_creds: Option<Vec<CredentialID>> = if existing.is_empty() {
        None
    } else {
        Some(existing.iter().map(|p| p.cred_id().clone().into()).collect())
    };

    let (ccr, reg_state) = WEBAUTHN
        .start_passkey_registration(user_uuid, &user.1, &user.1, exclude_creds)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("WebAuthn error: {e}")))?;

    let challenge_id = uuid::Uuid::new_v4().to_string();
    let state_json = serde_json::json!({
        "user_id": user_id,
        "state": serde_json::to_value(&reg_state).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Serialize error: {e}")))?
    });
    let now = chrono::Utc::now();
    let expires_at = now + chrono::Duration::minutes(5);

    sqlx::query(
        "INSERT INTO webauthn_challenges (challenge_id, challenge_type, state_json, created_at, expires_at) VALUES (?, 'registration', ?, ?, ?)"
    )
    .bind(&challenge_id)
    .bind(serde_json::to_string(&state_json).unwrap())
    .bind(now.to_rfc3339())
    .bind(expires_at.to_rfc3339())
    .execute(&*DB)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    Ok(Json(serde_json::json!({
        "challenge_id": challenge_id,
        "options": serde_json::to_value(&ccr).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Serialize error: {e}")))?
    })))
}

/// POST /api/auth/passkeys/add/finish
pub async fn add_passkey_finish(
    Extension(user_id): Extension<i64>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let challenge_id = body["challenge_id"]
        .as_str()
        .ok_or((StatusCode::BAD_REQUEST, "Missing challenge_id".to_string()))?;

    let row: Option<(String, String)> = sqlx::query_as(
        "SELECT state_json, expires_at FROM webauthn_challenges WHERE challenge_id = ? AND challenge_type = 'registration'"
    )
    .bind(challenge_id)
    .fetch_optional(&*DB)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    let (state_json_str, expires_at) = row.ok_or((StatusCode::BAD_REQUEST, "Challenge not found".to_string()))?;

    let expires = chrono::DateTime::parse_from_rfc3339(&expires_at)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Date parse error: {e}")))?;
    if chrono::Utc::now() > expires {
        return Err((StatusCode::BAD_REQUEST, "Challenge expired".to_string()));
    }

    let state_wrapper: serde_json::Value = serde_json::from_str(&state_json_str)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("JSON parse error: {e}")))?;

    let reg_state: PasskeyRegistration = serde_json::from_value(state_wrapper["state"].clone())
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Deserialize error: {e}")))?;

    let credential: RegisterPublicKeyCredential = serde_json::from_value(body["credential"].clone())
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid credential: {e}")))?;

    let passkey = WEBAUTHN
        .finish_passkey_registration(&credential, &reg_state)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Registration failed: {e}")))?;

    let cred_id_b64 = base64url_encode(passkey.cred_id());
    let passkey_json = serde_json::to_string(&passkey)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Serialize error: {e}")))?;

    let name = body["name"].as_str().unwrap_or("My Passkey");

    sqlx::query(
        "INSERT INTO credentials (user_id, credential_id, passkey_json, name) VALUES (?, ?, ?, ?)"
    )
    .bind(user_id)
    .bind(&cred_id_b64)
    .bind(&passkey_json)
    .bind(name)
    .execute(&*DB)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    // Delete challenge
    sqlx::query("DELETE FROM webauthn_challenges WHERE challenge_id = ?")
        .bind(challenge_id)
        .execute(&*DB)
        .await
        .ok();

    // Get newly created credential info
    let new_cred: (i64, String, String, Option<String>) = sqlx::query_as(
        "SELECT id, name, created_at, last_used FROM credentials WHERE credential_id = ?"
    )
    .bind(&cred_id_b64)
    .fetch_one(&*DB)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    Ok(Json(serde_json::json!({
        "id": new_cred.0,
        "name": new_cred.1,
        "created_at": new_cred.2,
        "last_used": new_cred.3
    })))
}

/// PUT /api/auth/passkeys/{id}/name
pub async fn rename_passkey(
    Extension(user_id): Extension<i64>,
    Path(passkey_id): Path<i64>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let name = body["name"]
        .as_str()
        .ok_or((StatusCode::BAD_REQUEST, "Missing name".to_string()))?;

    let result = sqlx::query(
        "UPDATE credentials SET name = ? WHERE id = ? AND user_id = ?"
    )
    .bind(name)
    .bind(passkey_id)
    .bind(user_id)
    .execute(&*DB)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    if result.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, "Passkey not found".to_string()));
    }

    Ok(Json(serde_json::json!({"status": "ok"})))
}

/// DELETE /api/auth/passkeys/{id}
pub async fn delete_passkey(
    Extension(user_id): Extension<i64>,
    Path(passkey_id): Path<i64>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // Check count — block deleting last passkey
    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM credentials WHERE user_id = ?"
    )
    .bind(user_id)
    .fetch_one(&*DB)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    if count.0 <= 1 {
        return Err((StatusCode::BAD_REQUEST, "Cannot delete your last passkey".to_string()));
    }

    let result = sqlx::query(
        "DELETE FROM credentials WHERE id = ? AND user_id = ?"
    )
    .bind(passkey_id)
    .bind(user_id)
    .execute(&*DB)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    if result.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, "Passkey not found".to_string()));
    }

    Ok(Json(serde_json::json!({"status": "ok"})))
}

fn base64url_encode(bytes: &[u8]) -> String {
    // Simple base64url encoding without padding
    let mut encoded = String::new();
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut i = 0;
    while i < bytes.len() {
        let b0 = bytes[i] as u32;
        let b1 = if i + 1 < bytes.len() { bytes[i + 1] as u32 } else { 0 };
        let b2 = if i + 2 < bytes.len() { bytes[i + 2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        encoded.push(ALPHABET[((triple >> 18) & 0x3F) as usize] as char);
        encoded.push(ALPHABET[((triple >> 12) & 0x3F) as usize] as char);
        if i + 1 < bytes.len() {
            encoded.push(ALPHABET[((triple >> 6) & 0x3F) as usize] as char);
        }
        if i + 2 < bytes.len() {
            encoded.push(ALPHABET[(triple & 0x3F) as usize] as char);
        }
        i += 3;
    }
    encoded
}
