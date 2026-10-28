//! Confidential OIDC client. Identity is the verified (issuer, subject), never a name/email.
use crate::{
    auth::jwt::{AuthUser, create_token},
    errors::ApiError,
    state::AppState,
};
use axum::{
    Json,
    extract::{Query, State},
    http::{HeaderMap, header},
    response::{IntoResponse, Redirect, Response},
};
use open_tappd_domain::crypto::{decrypt_field, encrypt_field};
use openidconnect::{
    AccessTokenHash, AuthorizationCode, ClientId, ClientSecret, CsrfToken, IssuerUrl, Nonce,
    OAuth2TokenResponse, PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope, TokenResponse,
    core::{CoreAuthenticationFlow, CoreClient, CoreProviderMetadata},
    reqwest,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub const ISSUER: &str = "https://sproyt-security.bjoroy.me/application/o/open-tappd/";
pub const SPROYT_ISSUER: &str = "https://sproyt-security.bjoroy.me/application/o/sproyt/";
const COOKIE: &str = "__Host-open_tappd_identity";

#[derive(Clone)]
pub struct IdentityConfig {
    pub client_id: String,
    pub client_secret: String,
    pub origin: String,
    pub http: reqwest::Client,
}
impl IdentityConfig {
    pub fn from_env() -> Result<Option<Self>, Box<dyn std::error::Error>> {
        let Ok(client_id) = std::env::var("OIDC_CLIENT_ID") else {
            return Ok(None);
        };
        let client_secret = std::env::var("OIDC_CLIENT_SECRET")?;
        let origin = std::env::var("OIDC_ORIGIN")?;
        let url = url::Url::parse(&origin)?;
        if client_id.is_empty()
            || client_secret.is_empty()
            || url.scheme() != "https"
            || url.origin().ascii_serialization() != origin
        {
            return Err("OIDC requires a client, secret and exact HTTPS origin".into());
        }
        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(std::time::Duration::from_secs(15))
            .build()?;
        Ok(Some(Self {
            client_id,
            client_secret,
            origin,
            http,
        }))
    }
    async fn metadata(&self) -> Result<CoreProviderMetadata, ApiError> {
        let meta = CoreProviderMetadata::discover_async(
            IssuerUrl::new(ISSUER.into()).unwrap(),
            &self.http,
        )
        .await
        .map_err(|_| unavailable())?;
        // Do not send a client secret to a discovery-controlled foreign host.
        for endpoint in [
            Some(meta.authorization_endpoint().url()),
            meta.token_endpoint().map(|u| u.url()),
            Some(meta.jwks_uri().url()),
        ]
        .into_iter()
        .flatten()
        {
            if endpoint.scheme() != "https"
                || endpoint.host_str() != Some("sproyt-security.bjoroy.me")
            {
                return Err(unavailable());
            }
        }
        Ok(meta)
    }
}
fn unavailable() -> ApiError {
    ApiError::ServiceUnavailable(
        "Identity service is unavailable. You can still use your recovery key or passkey.".into(),
    )
}
fn invalid() -> ApiError {
    ApiError::Unauthorized(
        "Identity verification expired or was already used. Please start again.".into(),
    )
}
fn config(state: &AppState) -> Result<&IdentityConfig, ApiError> {
    state.identity.as_ref().ok_or_else(unavailable)
}
fn hash(value: &str) -> Vec<u8> {
    Sha256::digest(value.as_bytes()).to_vec()
}
fn random() -> String {
    CsrfToken::new_random_len(32).secret().clone()
}
fn cookie(headers: &HeaderMap) -> Result<String, ApiError> {
    headers
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| {
            v.split(';')
                .find_map(|p| p.trim().strip_prefix(&format!("{COOKIE}=")))
        })
        .filter(|v| v.len() >= 32 && v.len() <= 128)
        .map(str::to_owned)
        .ok_or_else(invalid)
}
fn check_origin(headers: &HeaderMap, cfg: &IdentityConfig) -> Result<(), ApiError> {
    if headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) != Some(&cfg.origin) {
        return Err(invalid());
    }
    Ok(())
}
fn private_response(body: impl IntoResponse, cookie_value: Option<&str>) -> Response {
    let mut response = body.into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    response
        .headers_mut()
        .insert(header::REFERRER_POLICY, "no-referrer".parse().unwrap());
    if let Some(value) = cookie_value {
        response.headers_mut().insert(
            header::SET_COOKIE,
            format!(
                "{COOKIE}={value}; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age={}",
                if value.is_empty() { 0 } else { 600 }
            )
            .parse()
            .unwrap(),
        );
    }
    response
}
#[derive(Serialize, Deserialize)]
struct Payload {
    nonce: String,
    verifier: String,
    subject: Option<String>,
    display_name: Option<String>,
}
#[derive(sqlx::FromRow)]
struct Flow {
    user_id: Option<Uuid>,
    payload: Vec<u8>,
}
fn encode(p: &Payload, state: &AppState) -> Result<Vec<u8>, ApiError> {
    Ok(encrypt_field(
        &serde_json::to_string(p).map_err(|_| invalid())?,
        &state.encryption_key,
    )?)
}
fn decode(p: &[u8], state: &AppState) -> Result<Payload, ApiError> {
    serde_json::from_str(&decrypt_field(p, &state.encryption_key)?).map_err(|_| invalid())
}

pub async fn availability(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({"enabled":state.identity.is_some()}))
}
pub async fn login_start(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(_): Json<serde_json::Value>,
) -> Result<Response, ApiError> {
    start(&state, &headers, None).await
}
pub async fn link_start(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    Json(_): Json<serde_json::Value>,
) -> Result<Response, ApiError> {
    start(&state, &headers, Some(auth.user_id)).await
}
async fn start(
    state: &AppState,
    headers: &HeaderMap,
    user: Option<Uuid>,
) -> Result<Response, ApiError> {
    let cfg = config(state)?;
    check_origin(headers, cfg)?;
    let client = CoreClient::from_provider_metadata(
        cfg.metadata().await?,
        ClientId::new(cfg.client_id.clone()),
        Some(ClientSecret::new(cfg.client_secret.clone())),
    )
    .set_redirect_uri(RedirectUrl::new(format!("{}/api/identity/callback", cfg.origin)).unwrap());
    let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
    let (url, csrf, nonce) = client
        .authorize_url(
            CoreAuthenticationFlow::AuthorizationCode,
            CsrfToken::new_random,
            Nonce::new_random,
        )
        .add_scope(Scope::new("profile".into()))
        .set_pkce_challenge(challenge)
        .add_extra_param("prompt", "login")
        .url();
    let browser = random();
    let payload = encode(
        &Payload {
            nonce: nonce.secret().clone(),
            verifier: verifier.secret().clone(),
            subject: None,
            display_name: None,
        },
        state,
    )?;
    let mut tx = state.pool.begin().await?;
    sqlx::query("DELETE FROM identity_flows WHERE expires_at < now() OR browser_hash=$1")
        .bind(cookie(headers).map(|c| hash(&c)).unwrap_or_default())
        .execute(&mut *tx)
        .await?;
    // Bound storage under unauthenticated load; the endpoint fails closed at capacity.
    sqlx::query("SELECT pg_advisory_xact_lock(7319401)")
        .execute(&mut *tx)
        .await?;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM identity_flows")
        .fetch_one(&mut *tx)
        .await?;
    if count >= 1000 {
        return Err(unavailable());
    }
    sqlx::query("INSERT INTO identity_flows(state_hash,browser_hash,user_id,stage,payload,expires_at) VALUES($1,$2,$3,'authorize',$4,now()+interval '10 minutes')")
        .bind(hash(csrf.secret())).bind(hash(&browser)).bind(user).bind(payload).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(private_response(
        Json(serde_json::json!({"url":url.to_string()})),
        Some(&browser),
    ))
}
#[derive(Deserialize)]
pub struct Callback {
    state: String,
    code: Option<String>,
    error: Option<String>,
}
pub async fn callback(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<Callback>,
) -> Response {
    match callback_inner(&state, &headers, query).await {
        Ok(r) => r,
        Err(_) => private_response(Redirect::to("/identity/finish?failed=1"), Some("")),
    }
}
async fn callback_inner(
    state: &AppState,
    headers: &HeaderMap,
    query: Callback,
) -> Result<Response, ApiError> {
    let cfg = config(state)?;
    let browser = cookie(headers)?;
    let flow = sqlx::query_as::<_,Flow>("DELETE FROM identity_flows WHERE state_hash=$1 AND browser_hash=$2 AND stage='authorize' AND expires_at>now() RETURNING user_id,payload")
        .bind(hash(&query.state)).bind(hash(&browser)).fetch_optional(&state.pool).await?.ok_or_else(invalid)?;
    if query.error.is_some() {
        return Err(invalid());
    }
    let p = decode(&flow.payload, state)?;
    let client = CoreClient::from_provider_metadata(
        cfg.metadata().await?,
        ClientId::new(cfg.client_id.clone()),
        Some(ClientSecret::new(cfg.client_secret.clone())),
    )
    .set_redirect_uri(RedirectUrl::new(format!("{}/api/identity/callback", cfg.origin)).unwrap());
    let tokens = client
        .exchange_code(AuthorizationCode::new(query.code.ok_or_else(invalid)?))
        .map_err(|_| invalid())?
        .set_pkce_verifier(PkceCodeVerifier::new(p.verifier))
        .request_async(&cfg.http)
        .await
        .map_err(|_| invalid())?;
    let id_token = tokens.id_token().ok_or_else(invalid)?;
    let claims = verified_claims(id_token, client.id_token_verifier(), &Nonce::new(p.nonce))?;
    if let Some(expected) = claims.access_token_hash() {
        let actual = AccessTokenHash::from_token(
            tokens.access_token(),
            id_token.signing_alg().map_err(|_| invalid())?,
            id_token
                .signing_key(&client.id_token_verifier())
                .map_err(|_| invalid())?,
        )
        .map_err(|_| invalid())?;
        if actual != *expected {
            return Err(invalid());
        }
    }
    let subject = claims.subject().as_str().to_owned();
    if subject.is_empty() || subject.len() > 255 {
        return Err(invalid());
    }
    let display_name = claims
        .preferred_username()
        .map(|n| n.as_str())
        .unwrap_or("bjoroy.me account")
        .to_owned();
    let result = random();
    let payload = encode(
        &Payload {
            nonce: String::new(),
            verifier: String::new(),
            subject: Some(subject),
            display_name: Some(display_name),
        },
        state,
    )?;
    sqlx::query("INSERT INTO identity_flows(state_hash,browser_hash,user_id,stage,payload,expires_at) VALUES($1,$1,$2,'confirm',$3,now()+interval '5 minutes')")
        .bind(hash(&result)).bind(flow.user_id).bind(payload).execute(&state.pool).await?;
    Ok(private_response(
        Redirect::to("/identity/finish"),
        Some(&result),
    ))
}
pub async fn pending(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let flow = sqlx::query_as::<_,Flow>("SELECT user_id,payload FROM identity_flows WHERE state_hash=$1 AND stage='confirm' AND expires_at>now()")
        .bind(hash(&cookie(&headers)?)).fetch_optional(&state.pool).await?.ok_or_else(invalid)?;
    let p = decode(&flow.payload, &state)?;
    let username: Option<String> = if let Some(id) = flow.user_id {
        sqlx::query_scalar("SELECT username FROM users WHERE id=$1")
            .bind(id)
            .fetch_optional(&state.pool)
            .await?
    } else {
        None
    };
    Ok(private_response(
        Json(
            serde_json::json!({"linking":flow.user_id.is_some(),"identity_name":p.display_name,"username":username}),
        ),
        None,
    ))
}
/// Sprøyt's existing UUIDv5 mapping. Both reviewed Authentik providers use user.uid;
/// the different per-provider issuers are never treated as interchangeable for JWT verification.
pub fn sproyt_id(subject: &str) -> Uuid {
    Uuid::new_v5(
        &Uuid::NAMESPACE_URL,
        format!("{SPROYT_ISSUER}:{subject}").as_bytes(),
    )
}

fn verified_claims<'a>(
    token: &'a openidconnect::core::CoreIdToken,
    verifier: openidconnect::core::CoreIdTokenVerifier<'_>,
    nonce: &Nonce,
) -> Result<&'a openidconnect::core::CoreIdTokenClaims, ApiError> {
    let verifier = verifier
        .set_allowed_algs([openidconnect::core::CoreJwsSigningAlgorithm::RsaSsaPkcs1V15Sha256]);
    token.claims(&verifier, nonce).map_err(|_| invalid())
}

pub async fn complete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(_): Json<serde_json::Value>,
) -> Result<Response, ApiError> {
    check_origin(&headers, config(&state)?)?;
    // Consume before attempting the link, so failed/replayed confirmation cannot be reused.
    let flow = sqlx::query_as::<_,Flow>("DELETE FROM identity_flows WHERE state_hash=$1 AND stage='confirm' AND expires_at>now() RETURNING user_id,payload")
        .bind(hash(&cookie(&headers)?)).fetch_optional(&state.pool).await?.ok_or_else(invalid)?;
    let p = decode(&flow.payload, &state)?;
    let subject = p.subject.ok_or_else(invalid)?;
    let user_id = if let Some(user) = flow.user_id {
        let inserted = sqlx::query("INSERT INTO external_identities(user_id,issuer,subject,display_name,sproyt_user_id) VALUES($1,$2,$3,$4,$5) ON CONFLICT DO NOTHING")
            .bind(user).bind(ISSUER).bind(&subject).bind(p.display_name.unwrap_or_default()).bind(sproyt_id(&subject)).execute(&state.pool).await?;
        if inserted.rows_affected() == 0 {
            let same: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM external_identities WHERE user_id=$1 AND issuer=$2 AND subject=$3)")
                .bind(user).bind(ISSUER).bind(&subject).fetch_one(&state.pool).await?;
            if !same {
                return Err(ApiError::Conflict("This account or bjoroy.me identity is already linked. No accounts were merged.".into()));
            }
        }
        user
    } else {
        sqlx::query_scalar("SELECT user_id FROM external_identities WHERE issuer=$1 AND subject=$2")
            .bind(ISSUER).bind(&subject).fetch_optional(&state.pool).await?
            .ok_or_else(|| ApiError::Unauthorized("This bjoroy.me identity is not linked yet. Sign in with your recovery key or passkey (or create an account), then link it from My Profile.".into()))?
    };
    let user = crate::db::users::find_user_by_id(&state.pool, user_id)
        .await?
        .ok_or_else(invalid)?;
    let token = create_token(user.id, &user.username, &state.jwt_secret)?;
    Ok(private_response(
        Json(serde_json::json!({"token":token})),
        Some(""),
    ))
}
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct LinkedIdentity {
    pub issuer: String,
    pub subject: String,
    pub display_name: String,
    pub sproyt_user_id: Uuid,
    pub linked_at: chrono::DateTime<chrono::Utc>,
}
pub async fn status(State(state): State<AppState>, auth: AuthUser) -> Result<Response, ApiError> {
    let link = sqlx::query_as::<_,LinkedIdentity>("SELECT issuer,subject,display_name,sproyt_user_id,linked_at FROM external_identities WHERE user_id=$1")
        .bind(auth.user_id).fetch_optional(&state.pool).await?;
    Ok(private_response(
        Json(
            serde_json::json!({"enabled":state.identity.is_some(),"social_eligible":link.is_some(),"identity":link}),
        ),
        None,
    ))
}

#[cfg(test)]
mod tests;
