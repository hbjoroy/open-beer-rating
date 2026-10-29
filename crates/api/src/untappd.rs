//! Atomic, previewed import of the flat Untappd JSON export.
//! External IDs identify catalog entries globally and check-ins per account.
use std::collections::{HashMap, HashSet};

use axum::{Json, body::Bytes, extract::State, http::HeaderMap};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD as B64};
use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use open_tappd_domain::{crypto, models::tasting::ServingStyle};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{Acquire, Postgres, Transaction};
use uuid::Uuid;

use crate::{auth::jwt::AuthUser, errors::ApiError, state::AppState};

pub const MAX_BYTES: usize = 16 * 1024 * 1024;
const MAX_ROWS: usize = 10_000;
static IMPORT_SLOTS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);
type Tx<'a> = Transaction<'a, Postgres>;

#[derive(Clone, Deserialize, Serialize, PartialEq)]
struct Checkin {
    checkin_id: i64,
    bid: i64,
    brewery_id: i64,
    beer_name: String,
    brewery_name: String,
    #[serde(default)]
    beer_type: Option<String>,
    beer_abv: Option<f64>,
    brewery_country: Option<String>,
    brewery_city: Option<String>,
    #[serde(default, deserialize_with = "optional_score")]
    rating_score: Option<f64>,
    created_at: DateTime<Utc>,
    comment: Option<String>,
    serving_type: Option<String>,
    venue_id: Option<i64>,
    venue_name: Option<String>,
    venue_city: Option<String>,
    venue_country: Option<String>,
    venue_state: Option<String>,
    venue_lat: Option<f64>,
    venue_lng: Option<f64>,
    photo_url: Option<String>,
    beer_ibu: Option<f64>,
    purchase_venue: Option<String>,
    #[serde(default)]
    flavor_profiles: Vec<String>,
}

#[derive(Default, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CatalogCount {
    pub created: usize,
    pub reused: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ImportIssue {
    pub checkin_id: Option<i64>,
    pub name: String,
    pub message: String,
}

#[derive(Default, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ImportReport {
    pub rows: usize,
    pub imported: usize,
    pub already_imported: usize,
    pub duplicate_rows: usize,
    pub unrated_checkins: Vec<i64>,
    pub rounded_scores: usize,
    pub photo_links: usize,
    pub notices: Vec<ImportIssue>,
    pub skipped_checkins: Vec<ImportIssue>,
    pub omitted_venues: Vec<ImportIssue>,
    pub breweries: CatalogCount,
    pub beers: CatalogCount,
    pub locations: CatalogCount,
}

#[derive(Serialize)]
pub struct ImportResponse {
    pub report: ImportReport,
    pub preview_token: Option<String>,
    pub committed: bool,
}

pub async fn preview(
    State(state): State<AppState>,
    auth: AuthUser,
    body: Bytes,
) -> Result<Json<ImportResponse>, ApiError> {
    run(&state, &auth, &body, None).await.map(Json)
}

pub async fn commit(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<ImportResponse>, ApiError> {
    let receipt = headers
        .get("X-Import-Preview")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| ApiError::Validation("Preview this file before importing it.".into()))?;
    run(&state, &auth, &body, Some(receipt)).await.map(Json)
}

fn invalid(row: usize, field: &str) -> ApiError {
    ApiError::Validation(format!(
        "Row {}: invalid {field}. Nothing was imported.",
        row + 1
    ))
}

fn text_ok(value: Option<&str>, max: usize) -> bool {
    value.is_none_or(|v| v.chars().count() <= max && !v.contains('\0'))
}

fn parse(body: &[u8]) -> Result<Vec<Checkin>, ApiError> {
    if body.len() > MAX_BYTES {
        return Err(ApiError::Validation("The file exceeds 16 MiB.".into()));
    }
    let rows: Vec<Checkin> = serde_json::from_slice(
        body.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(body),
    )
    .map_err(|e| {
        ApiError::Validation(format!(
            "Expected an Untappd JSON array (invalid data at line {}, column {}).",
            e.line(),
            e.column()
        ))
    })?;
    if rows.is_empty() || rows.len() > MAX_ROWS {
        return Err(ApiError::Validation(
            "Choose an export with 1–10,000 check-ins.".into(),
        ));
    }
    for (i, r) in rows.iter().enumerate() {
        if r.checkin_id <= 0 || r.bid <= 0 || r.brewery_id <= 0 {
            return Err(invalid(i, "Untappd ID"));
        }
        for (name, value, max) in [
            ("beer name", Some(r.beer_name.as_str()), 200),
            ("brewery name", Some(r.brewery_name.as_str()), 200),
            ("beer style", r.beer_type.as_deref(), 100),
            ("brewery country", r.brewery_country.as_deref(), 100),
            ("brewery city", r.brewery_city.as_deref(), 100),
            ("venue name", r.venue_name.as_deref(), 200),
            ("venue city", r.venue_city.as_deref(), 100),
            ("venue country", r.venue_country.as_deref(), 100),
            ("venue state", r.venue_state.as_deref(), 100),
            ("comment", r.comment.as_deref(), 10_000),
            ("serving type", r.serving_type.as_deref(), 100),
            ("photo URL", r.photo_url.as_deref(), 2048),
            ("purchase venue", r.purchase_venue.as_deref(), 500),
        ] {
            if !text_ok(value, max) {
                return Err(invalid(i, name));
            }
        }
        if r.beer_name.trim().is_empty() || r.brewery_name.trim().is_empty() {
            return Err(invalid(i, "name"));
        }
        for (name, value, low, high) in [
            ("rating", r.rating_score, 0.0, 5.0),
            ("ABV", r.beer_abv, 0.0, 100.0),
            ("latitude", r.venue_lat, -90.0, 90.0),
            ("longitude", r.venue_lng, -180.0, 180.0),
            ("IBU", r.beer_ibu, 0.0, 10000.0),
        ] {
            if value.is_some_and(|v| !v.is_finite() || v < low || v > high) {
                return Err(invalid(i, name));
            }
        }
        if r.created_at > Utc::now() + chrono::Duration::days(1) {
            return Err(invalid(i, "future date"));
        }
        if r.venue_id.is_some_and(|id| id < 0)
            || (r.venue_id.unwrap_or(0) > 0 && clean(&r.venue_name).is_none())
            || (r.venue_id.unwrap_or(0) == 0 && clean(&r.venue_name).is_some())
        {
            return Err(invalid(i, "venue ID/name"));
        }
        if r.flavor_profiles.len() > 100 || r.flavor_profiles.iter().any(|v| !text_ok(Some(v), 200))
        {
            return Err(invalid(i, "flavor profiles"));
        }
    }
    Ok(rows)
}

fn clean(value: &Option<String>) -> Option<&str> {
    value.as_deref().map(str::trim).filter(|v| !v.is_empty())
}

// Untappd exports an unrated check-in as an empty string, not consistently null.
fn optional_score<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<f64>, D::Error> {
    match serde_json::Value::deserialize(d)? {
        serde_json::Value::Null => Ok(None),
        serde_json::Value::String(s) if s.trim().is_empty() => Ok(None),
        serde_json::Value::Number(n) => n
            .as_f64()
            .map(Some)
            .ok_or_else(|| serde::de::Error::custom("invalid score")),
        _ => Err(serde::de::Error::custom(
            "expected a number or empty rating",
        )),
    }
}

fn serving(value: Option<&str>) -> Option<ServingStyle> {
    Some(match value?.trim().to_ascii_lowercase().as_str() {
        "" => return None,
        "draft" | "draught" => ServingStyle::Draft,
        "bottle" => ServingStyle::Bottle,
        "can" => ServingStyle::Can,
        "cask" => ServingStyle::Cask,
        "crowler" => ServingStyle::Crowler,
        "growler" => ServingStyle::Growler,
        "nitro" => ServingStyle::Nitro,
        "taster" => ServingStyle::Taster,
        _ => ServingStyle::Other,
    })
}

fn receipt_mac(
    state: &AppState,
    user: Uuid,
    body: &[u8],
    plan: &[u8],
    expires: i64,
) -> Hmac<Sha256> {
    let mut mac = Hmac::<Sha256>::new_from_slice(state.jwt_secret.as_bytes()).expect("HMAC key");
    mac.update(b"untappd-preview-v1\0");
    mac.update(user.as_bytes());
    mac.update(&Sha256::digest(body));
    mac.update(plan);
    mac.update(&expires.to_be_bytes());
    mac
}

async fn run(
    state: &AppState,
    auth: &AuthUser,
    body: &[u8],
    receipt: Option<&str>,
) -> Result<ImportResponse, ApiError> {
    let _permit = IMPORT_SLOTS.try_acquire().map_err(|_| {
        ApiError::Conflict("An import is running. Please try again shortly.".into())
    })?;
    let rows = parse(body)?;
    let mut report = ImportReport {
        rows: rows.len(),
        ..Default::default()
    };
    let mut tx = state.pool.begin().await?;
    // Bound database waits; all catalog writers are serialized during matching.
    sqlx::query("SET LOCAL lock_timeout = '5s'")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL statement_timeout = '30s'")
        .execute(&mut *tx)
        .await?;
    let locked: bool = sqlx::query_scalar("SELECT pg_try_advisory_xact_lock(7291042801)")
        .fetch_one(&mut *tx)
        .await?;
    if !locked {
        return Err(ApiError::Conflict(
            "An import is running. Please try again shortly.".into(),
        ));
    }
    // Same lock order as deletion/recovery. Recheck the session under the lock.
    let epoch: Option<i64> =
        sqlx::query_scalar("SELECT auth_version FROM users WHERE id=$1 FOR UPDATE")
            .bind(auth.user_id)
            .fetch_optional(&mut *tx)
            .await?;
    if epoch != Some(auth.auth_version) {
        return Err(ApiError::Unauthorized("Please sign in again.".into()));
    }
    sqlx::query("LOCK TABLE breweries, beers, locations IN SHARE ROW EXCLUSIVE MODE")
        .execute(&mut *tx)
        .await?;
    let existing: HashSet<i64> =
        sqlx::query_scalar::<_, i64>("SELECT checkin_id FROM untappd_checkins WHERE user_id=$1")
            .bind(auth.user_id)
            .fetch_all(&mut *tx)
            .await?
            .into_iter()
            .collect();
    // A repeated export entry represents one event. Use its last copy rather
    // than asking users to edit a third-party export by hand.
    let mut last = HashMap::new();
    let mut conflicting_duplicates = HashSet::new();
    for (index, row) in rows.iter().enumerate() {
        if let Some(previous) = last.insert(row.checkin_id, index) {
            if rows[previous] != *row {
                conflicting_duplicates.insert(row.checkin_id);
            }
        }
    }
    report.duplicate_rows = rows.len() - last.len();
    let mut cache = CatalogCache::default();
    let mut plan = Sha256::new();
    for (index, row) in rows.iter().enumerate() {
        if last[&row.checkin_id] != index {
            continue;
        }
        if conflicting_duplicates.contains(&row.checkin_id) {
            report.notices.push(issue(row, "This check-in occurs more than once with different details. Only the last copy in this file is used."));
        }
        if existing.contains(&row.checkin_id) {
            report.already_imported += 1;
            continue;
        }
        let Some(score) = row.rating_score else {
            report.unrated_checkins.push(row.checkin_id);
            continue;
        };
        // Each check-in has a savepoint. An unresolved catalog match cannot leave
        // behind an unused brewery or prevent unrelated check-ins from importing.
        let mut row_tx = tx.begin().await?;
        match save_row(&mut row_tx, state, auth.user_id, row, &cache).await {
            Ok(changes) => {
                row_tx.commit().await?;
                for d in changes.decisions {
                    match d.kind {
                        "brewery" => {
                            cache.breweries.insert(d.source, d.id);
                            count(&mut report.breweries, d.created);
                        }
                        "beer" => {
                            cache
                                .beers
                                .insert(d.source, (d.id, d.parent.expect("beer brewery")));
                            count(&mut report.beers, d.created);
                        }
                        "location" => {
                            cache.places.insert(d.source, d.id);
                            count(&mut report.locations, d.created);
                        }
                        _ => unreachable!("known catalog kind"),
                    }
                    decision(&mut plan, d.kind.as_bytes(), d.source, d.id, d.created);
                    if let Some(message) = d.notice {
                        report.notices.push(ImportIssue {
                            checkin_id: None,
                            name: d.name,
                            message,
                        });
                    }
                }
                if let Some(omission) = changes.omitted_venue {
                    report.omitted_venues.push(omission);
                }
                report.imported += 1;
                if (score * 2.0 - (score * 2.0).round()).abs() > 0.000001 {
                    report.rounded_scores += 1;
                }
                if clean(&row.photo_url).is_some() {
                    report.photo_links += 1;
                }
            }
            Err(ApiError::Conflict(message)) => {
                row_tx.rollback().await?;
                report.skipped_checkins.push(issue(row, &message));
            }
            Err(error) => return Err(error),
        }
    }
    plan.update(serde_json::to_vec(&report).expect("serializable report"));
    let plan = plan.finalize();
    let preview_token = if let Some(token) = receipt {
        let mismatch = || {
            ApiError::Conflict("The preview expired, the file changed, or the catalog changed. Preview the file again.".into())
        };
        let (expiry, signature) = token.split_once('.').ok_or_else(mismatch)?;
        let expires: i64 = expiry.parse().map_err(|_| mismatch())?;
        if expires < Utc::now().timestamp() || expires > Utc::now().timestamp() + 1800 {
            return Err(mismatch());
        }
        let signature = B64.decode(signature).map_err(|_| mismatch())?;
        receipt_mac(state, auth.user_id, body, &plan, expires)
            .verify_slice(&signature)
            .map_err(|_| mismatch())?;
        tx.commit().await?;
        None
    } else {
        tx.rollback().await?;
        let expires = Utc::now().timestamp() + 1800;
        Some(format!(
            "{expires}.{}",
            B64.encode(
                receipt_mac(state, auth.user_id, body, &plan, expires)
                    .finalize()
                    .into_bytes()
            )
        ))
    };
    Ok(ImportResponse {
        report,
        preview_token,
        committed: receipt.is_some(),
    })
}

#[derive(Default)]
struct CatalogCache {
    breweries: HashMap<i64, Uuid>,
    beers: HashMap<i64, (Uuid, Uuid)>,
    places: HashMap<i64, Uuid>,
}
struct CatalogDecision {
    kind: &'static str,
    source: i64,
    id: Uuid,
    created: bool,
    parent: Option<Uuid>,
    name: String,
    notice: Option<String>,
}
#[derive(Default)]
struct RowChanges {
    decisions: Vec<CatalogDecision>,
    omitted_venue: Option<ImportIssue>,
}
fn issue(row: &Checkin, message: &str) -> ImportIssue {
    ImportIssue {
        checkin_id: Some(row.checkin_id),
        name: row.beer_name.clone(),
        message: message.into(),
    }
}
async fn save_row(
    tx: &mut Tx<'_>,
    state: &AppState,
    user: Uuid,
    row: &Checkin,
    cache: &CatalogCache,
) -> Result<RowChanges, ApiError> {
    let mut changes = RowChanges::default();
    let brewery = if let Some(id) = cache.breweries.get(&row.brewery_id) {
        *id
    } else {
        let (id, created) = resolve_brewery(tx, row).await?;
        changes.decisions.push(CatalogDecision {
            kind: "brewery",
            source: row.brewery_id,
            id,
            created,
            parent: None,
            name: row.brewery_name.clone(),
            notice: None,
        });
        id
    };
    let beer = if let Some((id, owner)) = cache.beers.get(&row.bid) {
        if *owner != brewery {
            return Err(ApiError::Conflict("This check-in links the same beer to a different brewery. It was skipped to avoid an incorrect link; the other check-ins can still be imported.".into()));
        }
        *id
    } else {
        let (id, created, notice) = resolve_beer(tx, row, brewery).await?;
        changes.decisions.push(CatalogDecision {
            kind: "beer",
            source: row.bid,
            id,
            created,
            parent: Some(brewery),
            name: row.beer_name.clone(),
            notice,
        });
        id
    };
    let location = if let Some(source) = row.venue_id.filter(|id| *id > 0) {
        if let Some(id) = cache.places.get(&source) {
            Some(*id)
        } else {
            match resolve_location(tx, row, user).await {
                Ok((id, created)) => {
                    changes.decisions.push(CatalogDecision {
                        kind: "location",
                        source,
                        id,
                        created,
                        parent: None,
                        name: clean(&row.venue_name).unwrap_or("").into(),
                        notice: None,
                    });
                    Some(id)
                }
                Err(ApiError::Conflict(_)) => {
                    changes.omitted_venue = Some(ImportIssue { checkin_id: Some(row.checkin_id), name: clean(&row.venue_name).unwrap_or("").into(), message: "Several venues match. The tasting is imported without a venue; the original venue details are preserved in your data export.".into() });
                    None
                }
                Err(error) => return Err(error),
            }
        }
    } else {
        None
    };
    let notes = clean(&row.comment)
        .map(|s| crypto::encrypt_field(s, &state.encryption_key))
        .transpose()?;
    let tasting: Uuid = sqlx::query_scalar("INSERT INTO tastings(user_id,beer_id,score,serving_style,notes_encrypted,location_id,tasted_at) VALUES($1,$2,$3,$4,$5,$6,$7) RETURNING id")
        .bind(user).bind(beer).bind((row.rating_score.expect("rated check-in") * 2.0).round() as i32)
        .bind(serving(clean(&row.serving_type))).bind(notes).bind(location).bind(row.created_at).fetch_one(&mut **tx).await?;
    let source = serde_json::json!({"checkin_id":row.checkin_id,"beer_id":row.bid,"brewery_id":row.brewery_id,
        "venue_id":row.venue_id,"rating_score":row.rating_score,"photo_url":row.photo_url,"beer_ibu":row.beer_ibu,
        "purchase_venue":row.purchase_venue,"flavor_profiles":row.flavor_profiles,"serving_type":row.serving_type,
        "beer_name":row.beer_name,"beer_style":row.beer_type,"beer_abv":row.beer_abv,
        "brewery_name":row.brewery_name,"brewery_country":row.brewery_country,"brewery_city":row.brewery_city,
        "venue_name":row.venue_name,"venue_city":row.venue_city,"venue_country":row.venue_country,
        "venue_state":row.venue_state,"venue_lat":row.venue_lat,"venue_lng":row.venue_lng});
    let encrypted = crypto::encrypt_field(&source.to_string(), &state.encryption_key)?;
    sqlx::query("INSERT INTO untappd_checkins(user_id,checkin_id,tasting_id,source_encrypted) VALUES($1,$2,$3,$4)")
        .bind(user).bind(row.checkin_id).bind(tasting).bind(encrypted).execute(&mut **tx).await?;
    Ok(changes)
}

fn count(count: &mut CatalogCount, created: bool) {
    if created {
        count.created += 1;
    } else {
        count.reused += 1;
    }
}
fn decision(plan: &mut Sha256, kind: &[u8], source: i64, id: Uuid, created: bool) {
    plan.update(kind);
    plan.update(source.to_be_bytes());
    plan.update([u8::from(created)]);
    if !created {
        plan.update(id.as_bytes());
    }
}

fn unique(ids: Vec<Uuid>, kind: &str, _source: i64) -> Result<Option<Uuid>, ApiError> {
    if ids.len() > 1 {
        return Err(ApiError::Conflict(format!(
            "Several existing {kind} entries match. This check-in was skipped to avoid linking it to the wrong drink. Your other check-ins can still be imported."
        )));
    }
    Ok(ids.first().copied())
}

async fn resolve_brewery(tx: &mut Tx<'_>, r: &Checkin) -> Result<(Uuid, bool), ApiError> {
    if let Some(id) = sqlx::query_scalar("SELECT id FROM breweries WHERE untappd_id=$1")
        .bind(r.brewery_id)
        .fetch_optional(&mut **tx)
        .await?
    {
        return Ok((id, false));
    }
    let matches = sqlx::query_scalar("SELECT id FROM breweries WHERE untappd_id IS NULL AND catalog_key(name)=catalog_key($1) AND (country IS NULL OR $2::text IS NULL OR catalog_key(country)=catalog_key($2)) AND (city IS NULL OR $3::text IS NULL OR catalog_key(city)=catalog_key($3)) LIMIT 2")
        .bind(r.brewery_name.trim()).bind(clean(&r.brewery_country)).bind(clean(&r.brewery_city)).fetch_all(&mut **tx).await?;
    if let Some(id) = unique(matches, "brewery", r.brewery_id)? {
        sqlx::query("UPDATE breweries SET untappd_id=$2 WHERE id=$1")
            .bind(id)
            .bind(r.brewery_id)
            .execute(&mut **tx)
            .await?;
        return Ok((id, false));
    }
    let id = sqlx::query_scalar(
        "INSERT INTO breweries(name,country,city,untappd_id) VALUES($1,$2,$3,$4) RETURNING id",
    )
    .bind(r.brewery_name.trim())
    .bind(clean(&r.brewery_country))
    .bind(clean(&r.brewery_city))
    .bind(r.brewery_id)
    .fetch_one(&mut **tx)
    .await?;
    Ok((id, true))
}

async fn resolve_beer(
    tx: &mut Tx<'_>,
    r: &Checkin,
    brewery: Uuid,
) -> Result<(Uuid, bool, Option<String>), ApiError> {
    if let Some((id, owner, name, style, abv)) =
        sqlx::query_as::<_, (Uuid, Uuid, String, Option<String>, Option<f64>)>(
            "SELECT id,brewery_id,name,style,abv FROM beers WHERE untappd_id=$1",
        )
        .bind(r.bid)
        .fetch_optional(&mut **tx)
        .await?
    {
        if owner != brewery {
            return Err(ApiError::Conflict("The existing Untappd beer link points to a different brewery. This check-in was skipped; your other check-ins can still be imported.".into()));
        }
        return Ok((id, false, metadata_notice(&name, style.as_deref(), abv, r)));
    }
    // A unique name within a brewery is enough to adopt an unlinked beer.
    // Style descriptions and ABV drift are descriptive differences, not identity
    // failures. Keep catalog values and preserve the source privately per tasting.
    let matches = sqlx::query_as::<_, (Uuid, String, Option<String>, Option<f64>)>("SELECT id,name,style,abv FROM beers WHERE untappd_id IS NULL AND brewery_id=$1 AND catalog_key(name)=catalog_key($2) LIMIT 2")
        .bind(brewery).bind(r.beer_name.trim()).fetch_all(&mut **tx).await?;
    if let Some(id) = unique(matches.iter().map(|m| m.0).collect(), "beer", r.bid)? {
        let (_, name, style, abv) = &matches[0];
        let notice = metadata_notice(name, style.as_deref(), *abv, r);
        sqlx::query("UPDATE beers SET untappd_id=$2 WHERE id=$1")
            .bind(id)
            .bind(r.bid)
            .execute(&mut **tx)
            .await?;
        return Ok((id, false, notice));
    }
    let id = sqlx::query_scalar("INSERT INTO beers(brewery_id,name,style,abv,untappd_id) VALUES($1,$2,$3,$4,$5) RETURNING id")
        .bind(brewery).bind(r.beer_name.trim()).bind(clean(&r.beer_type)).bind(r.beer_abv).bind(r.bid).fetch_one(&mut **tx).await?;
    Ok((id, true, None))
}
fn metadata_notice(
    name: &str,
    style: Option<&str>,
    abv: Option<f64>,
    r: &Checkin,
) -> Option<String> {
    let mut differences = Vec::new();
    if key(name) != key(&r.beer_name) {
        differences.push("name");
    }
    if style
        .zip(clean(&r.beer_type))
        .is_some_and(|(a, b)| key(a) != key(b))
    {
        differences.push("style description");
    }
    if abv
        .zip(r.beer_abv)
        .is_some_and(|(a, b)| (a - b).abs() > 0.05)
    {
        differences.push("ABV");
    }
    (!differences.is_empty()).then(|| format!("Reused the existing beer despite a different {}. Existing catalog details are kept; the original Untappd details are preserved with your tasting.", differences.join(" and ")))
}

fn key(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

async fn resolve_location(
    tx: &mut Tx<'_>,
    r: &Checkin,
    user: Uuid,
) -> Result<(Uuid, bool), ApiError> {
    let source = r.venue_id.expect("validated venue");
    if let Some(id) = sqlx::query_scalar("SELECT id FROM locations WHERE untappd_id=$1")
        .bind(source)
        .fetch_optional(&mut **tx)
        .await?
    {
        return Ok((id, false));
    }
    let name = clean(&r.venue_name).expect("validated venue name");
    let matches = sqlx::query_scalar("SELECT id FROM locations WHERE untappd_id IS NULL AND catalog_key(name)=catalog_key($1) AND (nullif(metadata->>'city','') IS NULL OR $2::text IS NULL OR catalog_key(metadata->>'city')=catalog_key($2)) AND (nullif(metadata->>'country','') IS NULL OR $3::text IS NULL OR catalog_key(metadata->>'country')=catalog_key($3)) LIMIT 2")
        .bind(name).bind(clean(&r.venue_city)).bind(clean(&r.venue_country)).fetch_all(&mut **tx).await?;
    if let Some(id) = unique(matches, "venue", source)? {
        sqlx::query("UPDATE locations SET untappd_id=$2 WHERE id=$1")
            .bind(id)
            .bind(source)
            .execute(&mut **tx)
            .await?;
        return Ok((id, false));
    }
    let metadata = serde_json::json!({"city":clean(&r.venue_city),"country":clean(&r.venue_country),
        "state":clean(&r.venue_state),"latitude":r.venue_lat.map(|v|v.to_string()),"longitude":r.venue_lng.map(|v|v.to_string())});
    let id = sqlx::query_scalar("INSERT INTO locations(name,location_type,metadata,created_by,untappd_id) VALUES($1,'other',$2,$3,$4) RETURNING id")
        .bind(name).bind(metadata).bind(user).bind(source).fetch_one(&mut **tx).await?;
    Ok((id, true))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn serving_and_name_normalization() {
        assert_eq!(serving(Some(" Draught ")), Some(ServingStyle::Draft));
        assert_eq!(serving(Some("")), None);
        assert_eq!(serving(Some("unknown")), Some(ServingStyle::Other));
        assert_eq!(key("  ØL  BRYGGERI "), "øl bryggeri");
    }
    #[test]
    fn rejects_non_export_payloads() {
        for bytes in [b"[]".as_slice(), b"{}", b"null", b"[{}]"] {
            assert!(parse(bytes).is_err());
        }
    }
}
