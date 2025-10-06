mod db;
mod llm;
mod models;
mod musicbrainz;
mod scheduler;
pub mod scraping;
mod utils;

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::Serialize;

use db::{PendingEvent, Store};
use llm::{fallback, fallback_preview, LLMComposer};
use models::Event;

const BUCKET_KEYS: [&str; 6] = ["DAY_OF", "LT_1W", "LT_2W", "LT_1M", "LT_2M", "GTE_2M"];

#[derive(Debug, Serialize)]
struct BucketItem {
    days_until: i64,
    event: models::Event,
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
async fn mark_events_posted(eventIds: Vec<String>) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        let store = Store::open_default().map_err(|e| e.to_string())?;
        for event_id in eventIds {
            store
                .mark_posted(&event_id)
                .map_err(|e| format!("mark posted failed: {e}"))?;
        }
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())??;

    Ok(())
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
        .invoke_handler(tauri::generate_handler![
            list_venues,
            scrape_all,
            scrape_venue,
            list_pending_buckets,
            preview_post,
            mark_events_posted
        ])
        .setup(|_| {
            Store::open_default().map_err(|e| -> Box<dyn std::error::Error> { Box::new(e) })?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
