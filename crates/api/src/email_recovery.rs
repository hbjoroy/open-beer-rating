//! Transitional recovery for the explicitly grandfathered test cohort.
use crate::{db, errors::ApiError, state::AppState};
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, header},
    response::{IntoResponse, Response},
};
use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor, message::Mailbox,
    transport::smtp::authentication::Credentials,
};
use open_tappd_domain::crypto::{decrypt_field, encrypt_field};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use uuid::Uuid;

#[derive(Clone)]
pub struct RecoveryConfig {
    pub until: DateTime<Utc>,
    pub origin: String,
    pub from: Mailbox,
    pub smtp: AsyncSmtpTransport<Tokio1Executor>,
}
impl RecoveryConfig {
    pub fn from_env() -> Result<Option<Self>, Box<dyn std::error::Error>> {
        let Ok(until) = std::env::var("EMAIL_RECOVERY_UNTIL") else {
            return Ok(None);
        };
        let until = DateTime::parse_from_rfc3339(&until)?.with_timezone(&Utc);
        let origin = std::env::var("EMAIL_RECOVERY_ORIGIN")?;
        let url = url::Url::parse(&origin)?;
        if url.scheme() != "https" || url.origin().ascii_serialization() != origin {
            return Err("Recovery requires an exact HTTPS origin".into());
        }
        let smtp =
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&std::env::var("SMTP_HOST")?)?
                .port(std::env::var("SMTP_PORT")?.parse()?)
                .credentials(Credentials::new(
                    std::env::var("SMTP_USERNAME")?,
                    std::env::var("SMTP_PASSWORD")?,
                ))
                .timeout(Some(std::time::Duration::from_secs(15)))
                .build();
        Ok(Some(Self {
            until,
            origin,
            from: std::env::var("SMTP_FROM")?.parse()?,
            smtp,
        }))
    }
}
fn config(state: &AppState) -> Result<&RecoveryConfig, ApiError> {
    state
        .email_recovery
        .as_ref()
        .filter(|c| c.until > Utc::now())
        .ok_or_else(|| {
            ApiError::ServiceUnavailable(
                "Email recovery is closed. Use your recovery key, passkey or bjoroy.me identity."
                    .into(),
            )
        })
}
fn origin(headers: &HeaderMap, cfg: &RecoveryConfig) -> Result<(), ApiError> {
    if headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) != Some(&cfg.origin) {
        return Err(invalid());
    }
    Ok(())
}
fn invalid() -> ApiError {
    ApiError::Unauthorized("Invalid or expired email code. Request a new code if needed.".into())
}
fn private(body: impl IntoResponse) -> Response {
    let mut r = body.into_response();
    r.headers_mut()
        .insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    r
}
fn code_mac(state: &AppState, user: Uuid, code: &str) -> Hmac<Sha256> {
    let mut mac = Hmac::<Sha256>::new_from_slice(state.jwt_secret.as_bytes())
        .expect("HMAC accepts any key size");
    mac.update(b"open-tappd-email-recovery-v1:");
    mac.update(user.as_bytes());
    mac.update(code.trim().to_uppercase().as_bytes());
    mac
}
#[derive(Serialize, Deserialize)]
struct Delivery {
    email: String,
    username: String,
    code: String,
}
#[derive(Deserialize)]
pub struct RequestCode {
    pub username: String,
    pub email: String,
}
#[derive(Deserialize)]
pub struct RedeemCode {
    pub username: String,
    pub code: String,
}

pub async fn availability(State(state): State<AppState>) -> Response {
    let cfg = config(&state).ok();
    private(Json(
        serde_json::json!({"enabled":cfg.is_some(),"until":cfg.map(|c|c.until)}),
    ))
}
pub async fn request_code(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<RequestCode>,
) -> Result<Response, ApiError> {
    let cfg = config(&state)?;
    origin(&headers, cfg)?;
    let start = tokio::time::Instant::now();
    if req.username.len() <= 100 && req.email.len() <= 320 {
        // SMTP is handled by a bounded database outbox, never by the request.
        enqueue(&state, &req).await?;
    }
    tokio::time::sleep_until(start + std::time::Duration::from_millis(350)).await;
    Ok(private(Json(
        serde_json::json!({"message":"If the username and registered email match an eligible test account, a code will arrive shortly. Check spam too. Please wait at least one minute before trying again."}),
    )))
}
async fn enqueue(state: &AppState, req: &RequestCode) -> Result<(), ApiError> {
    let Some(user) = db::users::find_user_by_username(&state.pool, req.username.trim()).await?
    else {
        return Ok(());
    };
    let Some(encrypted) = user.email_encrypted else {
        return Ok(());
    };
    let email = decrypt_field(&encrypted, &state.encryption_key)?;
    if !email.trim().eq_ignore_ascii_case(req.email.trim()) || email.parse::<Mailbox>().is_err() {
        return Ok(());
    }
    let mut tx = state.pool.begin().await?;
    let eligible:bool=sqlx::query_scalar("SELECT COALESCE(email_verified_at IS NOT NULL AND email_recovery_allowed_until>now(),false) FROM users WHERE id=$1 FOR UPDATE")
        .bind(user.id).fetch_optional(&mut *tx).await?.unwrap_or(false);
    if !eligible {
        return Ok(());
    }
    let limited:bool=sqlx::query_scalar("SELECT count(*) FILTER(WHERE requested_at>now()-interval '1 minute')>0 OR count(*) FILTER(WHERE requested_at>now()-interval '1 hour')>=3 OR count(*)>=5 FROM email_recovery_requests WHERE user_id=$1 AND requested_at>now()-interval '24 hours'")
        .bind(user.id).fetch_one(&mut *tx).await?;
    if limited {
        return Ok(());
    }
    let id = Uuid::new_v4();
    let code = crate::routes::users::generate_recovery_key();
    let payload = encrypt_field(
        &serde_json::to_string(&Delivery {
            email: email.trim().into(),
            username: user.username,
            code: code.clone(),
        })
        .map_err(|_| invalid())?,
        &state.encryption_key,
    )?;
    sqlx::query("DELETE FROM email_recovery_codes WHERE user_id=$1")
        .bind(user.id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM email_recovery_requests WHERE requested_at<now()-interval '30 days'")
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO email_recovery_requests(user_id) VALUES($1)")
        .bind(user.id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO email_recovery_codes(id,user_id,code_hash,expires_at) VALUES($1,$2,$3,now()+interval '20 minutes')")
        .bind(id).bind(user.id).bind(code_mac(state,user.id,&code).finalize().into_bytes().to_vec()).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO email_recovery_outbox(id,payload) VALUES($1,$2)")
        .bind(id)
        .bind(payload)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}
#[derive(sqlx::FromRow)]
struct CodeRow {
    id: Uuid,
    code_hash: Vec<u8>,
    attempts: i32,
}
pub async fn redeem_code(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<RedeemCode>,
) -> Result<Response, ApiError> {
    origin(&headers, config(&state)?)?;
    if req.username.len() > 100 || req.code.len() > 100 {
        return Err(invalid());
    }
    let user = db::users::find_user_by_username(&state.pool, req.username.trim())
        .await?
        .ok_or_else(invalid)?;
    let mut tx = state.pool.begin().await?;
    let eligible:bool=sqlx::query_scalar("SELECT COALESCE(email_verified_at IS NOT NULL AND email_recovery_allowed_until>now(),false) FROM users WHERE id=$1 FOR UPDATE")
        .bind(user.id).fetch_optional(&mut *tx).await?.unwrap_or(false);
    if !eligible {
        return Err(invalid());
    }
    let row=sqlx::query_as::<_,CodeRow>("SELECT id,code_hash,attempts FROM email_recovery_codes WHERE user_id=$1 AND expires_at>now() FOR UPDATE")
        .bind(user.id).fetch_optional(&mut *tx).await?.ok_or_else(invalid)?;
    if row.attempts >= 5
        || code_mac(&state, user.id, &req.code)
            .verify_slice(&row.code_hash)
            .is_err()
    {
        sqlx::query("UPDATE email_recovery_codes SET attempts=attempts+1 WHERE id=$1")
            .bind(row.id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        return Err(invalid());
    }
    let (key, hash) = tokio::task::spawn_blocking(|| {
        let key = crate::routes::users::generate_recovery_key();
        let hash = crate::routes::users::hash_recovery_key(&key)?;
        Ok::<_, ApiError>((key, hash))
    })
    .await
    .map_err(|_| invalid())??;
    sqlx::query("UPDATE users SET recovery_key_hash=$2,auth_version=auth_version+1,email_verified_at=now(),email_verification_source='email_code',updated_at=now() WHERE id=$1")
        .bind(user.id).bind(hash).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM email_recovery_codes WHERE user_id=$1")
        .bind(user.id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM identity_flows WHERE user_id=$1")
        .bind(user.id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    // Restricted email capability never authenticates normal API calls. Save the
    // replacement key first, then use the existing normal login flow explicitly.
    Ok(private(Json(
        serde_json::json!({"username":user.username,"recovery_key":key}),
    )))
}

pub fn start_worker(state: AppState) {
    tokio::spawn(async move {
        loop {
            let deadline = state.email_recovery.as_ref().map(|c| c.until);
            let _ =
                sqlx::query("DELETE FROM email_recovery_codes WHERE expires_at<now() OR $1<now()")
                    .bind(deadline)
                    .execute(&state.pool)
                    .await;
            let _ = sqlx::query(
                "DELETE FROM email_recovery_requests WHERE requested_at<now()-interval '30 days'",
            )
            .execute(&state.pool)
            .await;
            if config(&state).is_ok() {
                if deliver_one(&state).await.is_err() {
                    tracing::warn!("Email recovery delivery failed; a bounded retry is scheduled");
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    });
}
async fn deliver_one(state: &AppState) -> Result<(), ApiError> {
    let cfg = config(state)?;
    let mut tx = state.pool.begin().await?;
    sqlx::query("DELETE FROM email_recovery_codes WHERE expires_at<now()")
        .execute(&mut *tx)
        .await?;
    let row:Option<(Uuid,Vec<u8>)>=sqlx::query_as("SELECT id,payload FROM email_recovery_outbox WHERE next_attempt_at<=now() AND attempts<3 ORDER BY next_attempt_at LIMIT 1 FOR UPDATE SKIP LOCKED").fetch_optional(&mut *tx).await?;
    let Some((id, payload)) = row else {
        tx.commit().await?;
        return Ok(());
    };
    let delivery: Delivery = serde_json::from_str(&decrypt_field(&payload, &state.encryption_key)?)
        .map_err(|_| invalid())?;
    let message=Message::builder().from(cfg.from.clone()).to(delivery.email.parse().map_err(|_|invalid())?)
        .subject("Open Drinks: your one-time recovery code")
        .body(format!("A recovery code was requested for Open Drinks account {}.\n\n{}\n\nEnter this code at {}/email-recovery\nIt expires in 20 minutes and can be used once. You will receive a new recovery key; the old key and existing login sessions will stop working.\n\nAfter saving your new key, please link your bjoroy.me identity from My Profile. Email recovery is a temporary test-account facility.\n\nIf you did not request this, ignore this email. Your account has not been changed. Never share this code.\n",delivery.username,delivery.code,cfg.origin))
        .map_err(|_|invalid())?;
    let result = cfg.smtp.send(message).await;
    if result.is_ok() {
        sqlx::query("DELETE FROM email_recovery_outbox WHERE id=$1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
    } else {
        sqlx::query("UPDATE email_recovery_outbox SET attempts=attempts+1,next_attempt_at=now()+interval '1 minute' WHERE id=$1").bind(id).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    result
        .map(|_| ())
        .map_err(|_| ApiError::ServiceUnavailable("Email delivery unavailable".into()))
}

#[cfg(test)]
mod tests;
