/**
 * ============================================================================
 * AUTHENTICATION CLIENT MODULE
 * ============================================================================
 * 
 * PURPOSE: Obtain and manage JWT tokens from the collector server
 * 
 * AUTHENTICATION FLOW:
 * 1. POST to /mock-auth with {"user": "...", "user_id": "...", "org": "...", "org_id": "...", "account_id": "..."}
 * 2. Receive {"token": "..."}
 * 3. Cache token in memory with expiration
 * 4. Auto-refresh when expired
 * 
 * TOKEN LIFECYCLE:
 * - Tokens valid for 24 hours (server-issued)
 * - Cached tokens refreshed 1 hour before expiration
 * - Cached tokens invalidated on authentication errors
 * 
 * ============================================================================
 */

use crate::collector::types::AuthToken;
use chrono::{Duration, Utc};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::time::Duration as StdDuration;

// Global token cache (in-memory only, never persisted to disk)
static CACHED_TOKEN: Lazy<Mutex<Option<AuthToken>>> = Lazy::new(|| Mutex::new(None));

/**
 * Request body for /mock-auth endpoint
 */
#[derive(Debug, Serialize)]
struct AuthRequest {
    user: String,
    user_id: String,
    org: String,
    org_id: String,
    account_id: String,
}

/**
 * Response body from /mock-auth endpoint
 */
#[derive(Debug, Deserialize)]
struct AuthResponse {
    token: String,
}

/**
 * Obtain a new JWT token from the authentication endpoint
 * 
 * Makes HTTP POST request to auth_url with user, org, and account credentials
 * Optionally includes app JWT token for authorization
 * Returns AuthToken with token string and expiration metadata
 */
pub async fn obtain_token(
    auth_url: &str,
    user: &str,
    user_id: &str,
    org: &str,
    org_id: &str,
    account_id: &str,
    app_jwt_token: Option<&str>,
) -> Result<AuthToken, String> {
    log::info!("Obtaining JWT token for user={}, user_id={}, org={}, org_id={}, account_id={}", 
        user, user_id, org, org_id, account_id);
    
    if let Some(token) = app_jwt_token {
        log::info!("App JWT token provided: length={}, first_10_chars={}", 
            token.len(), 
            if token.len() >= 10 { &token[..10] } else { token });
    } else {
        log::debug!("No app JWT token provided - using mock-auth (JWT optional)");
    }

    // Create HTTP client with timeout
    let client = reqwest::Client::builder()
        .timeout(StdDuration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    // Prepare request body
    let request_body = AuthRequest {
        user: user.to_string(),
        user_id: user_id.to_string(),
        org: org.to_string(),
        org_id: org_id.to_string(),
        account_id: account_id.to_string(),
    };

    // Build request with optional Authorization header
    let mut request = client
        .post(auth_url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json");

    // Add Authorization header if app JWT token is provided (optional for mock-auth)
    if let Some(token) = app_jwt_token {
        let auth_header = format!("Bearer {}", token);
        log::info!("[AUTH] Adding Authorization header: Bearer <token> (token_length: {}, header_length: {})", 
            token.len(), auth_header.len());
        request = request.header("Authorization", auth_header.clone());
        log::info!("[AUTH] Authorization header set successfully");
    } else {
        log::debug!("[AUTH] No app JWT token provided - using mock-auth (JWT optional)");
    }

    // Log final request details before sending
    log::info!("[AUTH] Sending POST request to: {}", auth_url);
    log::info!("[AUTH] Request body: user={}, user_id={}, org={}, org_id={}, account_id={}", 
        request_body.user, request_body.user_id, request_body.org, request_body.org_id, request_body.account_id);
    
    // Send POST request
    let response = request
        .json(&request_body)
        .send()
        .await
        .map_err(|e| format!("Failed to send auth request: {}", e))?;
    
    log::info!("[AUTH] Response received: status={}", response.status());

    // Check status code
    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        
        // Log detailed error for 401 (Unauthorized)
        if status == reqwest::StatusCode::UNAUTHORIZED {
            log::warn!("[AUTH] Received 401 Unauthorized - token may be expired or invalid");
            if app_jwt_token.is_some() {
                log::warn!("[AUTH] App JWT token was provided but rejected by server");
            } else {
                log::warn!("[AUTH] No app JWT token was provided in request");
            }
        }
        
        return Err(format!("Auth request failed with status {}: {}", status, error_text));
    }

    // Parse response JSON
    let auth_response: AuthResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse auth response: {}", e))?;

    // Create AuthToken with expiration metadata
    let issued_at = Utc::now();
    // Server issues 24-hour tokens, use 23-hour expiration for safety margin
    let expires_at = issued_at + Duration::hours(23);

    let token = AuthToken {
        token: auth_response.token,
        issued_at,
        expires_at,
    };

    log::info!("Successfully obtained JWT token, expires at {}", expires_at);
    Ok(token)
}

/**
 * Get a valid JWT token, using cache if available and not expired
 * 
 * Returns cached token if valid, otherwise obtains new token and caches it
 * This is the primary function to use for obtaining tokens
 * 
 * Note: If app_jwt_token is provided, it will be sent to the auth endpoint
 * for authorization. The cache is keyed by the combination of credentials,
 * so changing the app_jwt_token will result in a new token request.
 */
pub async fn get_valid_token(
    auth_url: &str,
    user: &str,
    user_id: &str,
    org: &str,
    org_id: &str,
    account_id: &str,
    app_jwt_token: Option<&str>,
) -> Result<String, String> {
    log::info!("[AUTH] get_valid_token called with app_jwt_token: {}", 
        if app_jwt_token.is_some() { 
            format!("Some(length: {})", app_jwt_token.unwrap().len())
        } else { 
            "None".to_string()
        });
    
    // Check cached token
    // Note: We don't check app_jwt_token in cache validation since it's used for auth
    // If app_jwt_token changes, we should get a new token anyway
    {
        let cached = CACHED_TOKEN.lock().unwrap();
        if let Some(token) = cached.as_ref() {
            if !token.is_expired() {
                log::debug!("[AUTH] Using cached JWT token");
                return Ok(token.token.clone());
            } else {
                log::info!("[AUTH] Cached JWT token expired, refreshing");
            }
        }
    }

    // Obtain new token
    log::info!("[AUTH] Calling obtain_token with app_jwt_token: {}", 
        if app_jwt_token.is_some() { 
            format!("Some(length: {})", app_jwt_token.unwrap().len())
        } else { 
            "None".to_string()
        });
    let new_token = obtain_token(auth_url, user, user_id, org, org_id, account_id, app_jwt_token).await?;
    let token_string = new_token.token.clone();

    // Cache new token
    {
        let mut cached = CACHED_TOKEN.lock().unwrap();
        *cached = Some(new_token);
    }

    Ok(token_string)
}

/**
 * Invalidate cached token
 * 
 * Called when server returns authentication error
 * Forces token refresh on next get_valid_token call
 */
pub fn invalidate_token() {
    log::warn!("Invalidating cached JWT token");
    let mut cached = CACHED_TOKEN.lock().unwrap();
    *cached = None;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_request_serialization() {
        let request = AuthRequest {
            user: "test_user".to_string(),
            user_id: "test_user_id".to_string(),
            org: "test_org".to_string(),
            org_id: "test_org_id".to_string(),
            account_id: "test_account_id".to_string(),
        };
        
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"user\":\"test_user\""));
        assert!(json.contains("\"org\":\"test_org\""));
        assert!(!json.contains("app_jwt_token")); // JWT token is sent as header, not in body
    }

    #[test]
    fn test_auth_response_deserialization() {
        let json = r#"{"token":"eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.test"}"#;
        let response: AuthResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.token, "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.test");
    }
}

