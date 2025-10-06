mod config;
mod db;
mod facebook;
mod llm;
mod models;
mod musicbrainz;
mod scheduler;
pub mod scraping;
mod utils;

use std::{collections::HashMap, convert::TryFrom};

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use tauri::State;

use config::{AppConfig, ConfigStore};
use db::{PendingEvent, Store};
use facebook::FbPoster;
use llm::{fallback, fallback_preview, LLMComposer};
use models::Event;

const BUCKET_KEYS: [&str; 6] = ["DAY_OF", "LT_1W", "LT_2W", "LT_1M", "LT_2M", "GTE_2M"];

fn facebook_status_from(config: &AppConfig) -> FacebookStatusData {
    FacebookStatusData {
        connected: config.facebook_access_token.is_some(),
        group_id: config.facebook_group_id.clone(),
        user_name: config.facebook_user_name.clone(),
        expires_at: config.facebook_token_expires_at.clone(),
    }
}

#[derive(Debug, Deserialize)]
struct AccessTokenResponse {
    access_token: String,
    expires_in: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct ListGroupsResponse {
    data: Vec<GroupItem>,
    paging: Option<Paging>,
}

#[derive(Debug, Deserialize)]
struct Paging {
    next: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GroupItem {
    id: String,
    name: String,
    administrator: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct UserResponse {
    id: String,
    name: Option<String>,
}

#[derive(Debug, Serialize)]
struct BucketItem {
    days_until: i64,
    event: models::Event,
}

#[derive(Debug, Serialize, Clone)]
struct FacebookStatusData {
    connected: bool,
    group_id: Option<String>,
    user_name: Option<String>,
    expires_at: Option<String>,
}

#[derive(Debug, Serialize)]
struct FacebookGroupData {
    id: String,
    name: String,
    administrator: bool,
}

#[tauri::command]
async fn facebook_status(
    config_store: State<'_, ConfigStore>,
) -> Result<FacebookStatusData, String> {
    Ok(facebook_status_from(&config_store.read()))
}

#[tauri::command]
async fn facebook_oauth_url(app_id: String, redirect_uri: String) -> Result<String, String> {
    let app_id = app_id.trim();
    let redirect_uri = redirect_uri.trim();
    if app_id.is_empty() {
        return Err("Facebook App ID is required".into());
    }
    if redirect_uri.is_empty() {
        return Err("Redirect URI is required".into());
    }

    let mut url = reqwest::Url::parse("https://www.facebook.com/v19.0/dialog/oauth")
        .map_err(|e| e.to_string())?;
    url.query_pairs_mut()
        .append_pair("client_id", app_id)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair(
            "scope",
            "public_profile,publish_to_groups,groups_access_member_info",
        )
        .append_pair("response_type", "code")
        .append_pair("auth_type", "rerequest");

    Ok(url.into())
}

#[tauri::command]
async fn facebook_complete_oauth(
    app_id: String,
    app_secret: String,
    redirect_uri: String,
    code: String,
    config_store: State<'_, ConfigStore>,
) -> Result<FacebookStatusData, String> {
    let app_id = app_id.trim().to_string();
    let app_secret = app_secret.trim().to_string();
    let redirect_uri = redirect_uri.trim().to_string();
    let code = code.trim().to_string();

    if app_id.is_empty() || app_secret.is_empty() || redirect_uri.is_empty() || code.is_empty() {
        return Err("App ID, App Secret, redirect URI, and code are required".into());
    }

    let client = reqwest::Client::new();
    let token_url = reqwest::Url::parse_with_params(
        "https://graph.facebook.com/v19.0/oauth/access_token",
        [
            ("client_id", app_id.as_str()),
            ("redirect_uri", redirect_uri.as_str()),
            ("client_secret", app_secret.as_str()),
            ("code", code.as_str()),
        ],
    )
    .map_err(|e| e.to_string())?;

    let token_response = client
        .get(token_url)
        .send()
        .await
        .map_err(|e| format!("facebook token request failed: {e}"))?;
    let status = token_response.status();
    let body = token_response
        .text()
        .await
        .map_err(|e| format!("facebook token decode failed: {e}"))?;
    if !status.is_success() {
        return Err(format!("facebook token error: {body}"));
    }

    let token_data: AccessTokenResponse =
        serde_json::from_str(&body).map_err(|e| format!("facebook token parse failed: {e}"))?;
    let mut access_token = token_data.access_token;
    let mut expires_in = token_data.expires_in;

    let exchange_url = reqwest::Url::parse_with_params(
        "https://graph.facebook.com/v19.0/oauth/access_token",
        [
            ("grant_type", "fb_exchange_token"),
            ("client_id", app_id.as_str()),
            ("client_secret", app_secret.as_str()),
            ("fb_exchange_token", access_token.as_str()),
        ],
    )
    .map_err(|e| e.to_string())?;

    if let Ok(exchange_response) = client.get(exchange_url).send().await {
        let exchange_status = exchange_response.status();
        match exchange_response.text().await {
            Ok(exchange_body) if exchange_status.is_success() => {
                if let Ok(exchange_data) =
                    serde_json::from_str::<AccessTokenResponse>(&exchange_body)
                {
                    access_token = exchange_data.access_token;
                    if exchange_data.expires_in.is_some() {
                        expires_in = exchange_data.expires_in;
                    }
                }
            }
            Ok(exchange_body) => {
                eprintln!("facebook token exchange error: {exchange_body}");
            }
            Err(err) => {
                eprintln!("facebook token exchange decode failed: {err}");
            }
        }
    }

    let mut me_url =
        reqwest::Url::parse("https://graph.facebook.com/v19.0/me").map_err(|e| e.to_string())?;
    me_url
        .query_pairs_mut()
        .append_pair("fields", "id,name")
        .append_pair("access_token", &access_token);

    let me_response = client
        .get(me_url)
        .send()
        .await
        .map_err(|e| format!("facebook profile request failed: {e}"))?;
    let me_status = me_response.status();
    let me_body = me_response
        .text()
        .await
        .map_err(|e| format!("facebook profile decode failed: {e}"))?;
    if !me_status.is_success() {
        return Err(format!("facebook profile error: {me_body}"));
    }

    let user: UserResponse = serde_json::from_str(&me_body)
        .map_err(|e| format!("facebook profile parse failed: {e}"))?;

    let expires_at = expires_in.and_then(|seconds| {
        i64::try_from(seconds)
            .ok()
            .map(|secs| (Utc::now() + Duration::seconds(secs)).to_rfc3339())
    });

    let user_id = user.id.clone();
    let user_name = user.name.clone();
    let expires_at_clone = expires_at.clone();
    let updated = config_store.update(|config| {
        config.facebook_access_token = Some(access_token.clone());
        config.facebook_token_expires_at = expires_at_clone.clone();
        config.facebook_user_id = Some(user_id.clone());
        config.facebook_user_name = user_name.clone();
    })?;

    Ok(facebook_status_from(&updated))
}

#[tauri::command]
async fn facebook_list_groups(
    config_store: State<'_, ConfigStore>,
) -> Result<Vec<FacebookGroupData>, String> {
    let config = config_store.read();
    let token = config
        .facebook_access_token
        .as_ref()
        .ok_or_else(|| "Facebook account is not connected".to_string())?;

    let mut url = reqwest::Url::parse("https://graph.facebook.com/v19.0/me/groups")
        .map_err(|e| e.to_string())?;
    url.query_pairs_mut()
        .append_pair("fields", "id,name,administrator")
        .append_pair("limit", "200")
        .append_pair("access_token", token);

    let client = reqwest::Client::new();
    let mut groups = Vec::new();
    let mut next_url = Some(url);

    while let Some(page_url) = next_url.take() {
        let response = client
            .get(page_url.clone())
            .send()
            .await
            .map_err(|e| format!("facebook group request failed: {e}"))?;
        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| format!("facebook group decode failed: {e}"))?;
        if !status.is_success() {
            return Err(format!("facebook group error: {body}"));
        }

        let page: ListGroupsResponse =
            serde_json::from_str(&body).map_err(|e| format!("facebook group parse failed: {e}"))?;

        groups.extend(page.data.into_iter().map(|item| FacebookGroupData {
            id: item.id,
            name: item.name,
            administrator: item.administrator.unwrap_or(false),
        }));

        next_url = page
            .paging
            .and_then(|paging| paging.next)
            .and_then(|next| reqwest::Url::parse(&next).ok());
    }

    Ok(groups)
}

#[tauri::command]
async fn facebook_set_group(
    group_id: String,
    config_store: State<'_, ConfigStore>,
) -> Result<FacebookStatusData, String> {
    let group_id = group_id.trim();
    if group_id.is_empty() {
        return Err("Group ID is required".into());
    }

    let updated = config_store.update(|config| {
        config.facebook_group_id = Some(group_id.to_string());
    })?;

    Ok(facebook_status_from(&updated))
}

#[tauri::command]
async fn facebook_disconnect(config_store: State<'_, ConfigStore>) -> Result<(), String> {
    config_store.update(|config| {
        config.facebook_access_token = None;
        config.facebook_token_expires_at = None;
        config.facebook_user_id = None;
        config.facebook_user_name = None;
        config.facebook_group_id = None;
    })?;
    Ok(())
}

#[tauri::command]
async fn list_venues() -> Result<Vec<scraping::ScraperInfo>, String> {
    Ok(scraping::list_scrapers())
}

#[tauri::command]
async fn scrape_all() -> Result<usize, String> {
    let events = tauri::async_runtime::spawn_blocking(scraping::run_all)
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    persist_events(events).await
}

#[tauri::command]
async fn scrape_venue(venue_id: String) -> Result<usize, String> {
    let events = tauri::async_runtime::spawn_blocking(move || scraping::run_single(&venue_id))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    persist_events(events).await
}

#[tauri::command]
async fn list_pending_buckets() -> Result<HashMap<&'static str, Vec<BucketItem>>, String> {
    let pending = tauri::async_runtime::spawn_blocking(|| -> Result<Vec<PendingEvent>, String> {
        let store = Store::open_default().map_err(|e| e.to_string())?;
        store.list_pending_events().map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())??;

    let mut enriched_events: Vec<Event> = Vec::with_capacity(pending.len());
    for item in pending {
        let event = item.event;
        match musicbrainz::enrich_event(event.clone()).await {
            Ok(enriched) => enriched_events.push(enriched),
            Err(err) => {
                eprintln!("musicbrainz enrich failed: {err}");
                enriched_events.push(event);
            }
        }
    }

    let now = Utc::now();
    let mut buckets: HashMap<&'static str, Vec<BucketItem>> =
        BUCKET_KEYS.iter().map(|key| (*key, Vec::new())).collect();

    for event in enriched_events {
        let start = match parse_start(&event) {
            Some(dt) => dt,
            None => continue,
        };
        let duration = start.signed_duration_since(now);
        if duration.num_seconds() < 0 {
            continue;
        }
        let days_until = duration.num_seconds() / 86_400;
        let bucket = bucket_for(days_until);
        if let Some(b) = buckets.get_mut(bucket) {
            b.push(BucketItem { days_until, event });
        }
    }

    for bucket in buckets.values_mut() {
        bucket.sort_by_key(|item| parse_start(&item.event).unwrap_or(now));
    }

    Ok(buckets)
}

#[allow(non_snake_case)]
#[tauri::command]
async fn preview_post(eventId: String) -> Result<String, String> {
    let event = tauri::async_runtime::spawn_blocking(move || -> Result<models::Event, String> {
        let store = Store::open_default().map_err(|e| e.to_string())?;
        store
            .get_event(&eventId)
            .map_err(|e| format!("event lookup failed: {e}"))
    })
    .await
    .map_err(|e| e.to_string())??;

    let event_for_prompt = match musicbrainz::enrich_event(event.clone()).await {
        Ok(enriched) => enriched,
        Err(err) => {
            eprintln!("musicbrainz enrich failed: {err}");
            event.clone()
        }
    };

    let composer = LLMComposer::from_env();
    match composer.compose_preview(&event_for_prompt).await {
        Ok(s) => Ok(s),
        Err(_) => Ok(fallback_preview(&event_for_prompt)),
    }
}

#[allow(non_snake_case)]
#[tauri::command]
async fn post_to_facebook(
    eventId: String,
    config_store: State<'_, ConfigStore>,
) -> Result<String, String> {
    let event = tauri::async_runtime::spawn_blocking(move || -> Result<models::Event, String> {
        let store = Store::open_default().map_err(|e| e.to_string())?;
        store
            .get_event(&eventId)
            .map_err(|e| format!("event lookup failed: {e}"))
    })
    .await
    .map_err(|e| e.to_string())??;

    let event_for_prompt = match musicbrainz::enrich_event(event.clone()).await {
        Ok(enriched) => enriched,
        Err(err) => {
            eprintln!("musicbrainz enrich failed: {err}");
            event.clone()
        }
    };

    let composer = LLMComposer::from_env();
    let message = match composer.compose(&event_for_prompt).await {
        Ok(msg) => msg,
        Err(_) => fallback(&event_for_prompt),
    };

    let config = config_store.read();
    let poster =
        FbPoster::from_config(&config).map_err(|e| format!("facebook configuration error: {e}"))?;
    let fb_id = poster
        .post(&message)
        .await
        .map_err(|e| format!("facebook error: {e}"))?;

    let event_id_clone = event.id.clone();
    let fb_id_clone = fb_id.clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        let store = Store::open_default().map_err(|e| e.to_string())?;
        store
            .mark_posted(&event_id_clone, &fb_id_clone)
            .map_err(|e| format!("mark posted failed: {e}"))
    })
    .await
    .map_err(|e| e.to_string())??;

    Ok(fb_id)
}

async fn persist_events(events: Vec<Event>) -> Result<usize, String> {
    if events.is_empty() {
        return Ok(0);
    }

    tauri::async_runtime::spawn_blocking(move || -> Result<usize, String> {
        let store = Store::open_default().map_err(|e| e.to_string())?;
        for event in &events {
            store
                .upsert_event(event)
                .map_err(|e| format!("failed to persist event {}: {e}", event.id))?;
        }
        Ok(events.len())
    })
    .await
    .map_err(|e| e.to_string())?
}

fn parse_start(event: &models::Event) -> Option<DateTime<Utc>> {
    chrono::DateTime::parse_from_rfc3339(&event.start_utc)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

fn bucket_for(days_until: i64) -> &'static str {
    match days_until {
        d if d <= 0 => "DAY_OF",
        d if d < 7 => "LT_1W",
        d if d < 14 => "LT_2W",
        d if d < 30 => "LT_1M",
        d if d < 60 => "LT_2M",
        _ => "GTE_2M",
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    scheduler::init();
    tauri::Builder::default()
        .manage(ConfigStore::load())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            list_venues,
            scrape_all,
            scrape_venue,
            list_pending_buckets,
            preview_post,
            facebook_status,
            facebook_oauth_url,
            facebook_complete_oauth,
            facebook_list_groups,
            facebook_set_group,
            facebook_disconnect,
            post_to_facebook
        ])
        .setup(|_| {
            Store::open_default().map_err(|e| -> Box<dyn std::error::Error> { Box::new(e) })?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
