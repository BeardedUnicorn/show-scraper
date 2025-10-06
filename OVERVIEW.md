# Tauri v2 ShowScraper – React + TypeScript App Plan

A desktop app that scrapes multiple music-venue sites, normalizes events, dedupes & persists them, composes a Facebook-group post via an **OpenAI‑compatible API**, and now hands drafts to humans for manual Facebook publishing. Built with **Tauri v2 (Rust backend)** and **React + TypeScript** frontend.

> **Update:** The automated Facebook Graph API integration has been removed. Drafts are generated locally and operators copy them into Facebook groups before marking events as posted.

---

## 1) Core features

- Venue adapters (HTML, ICS, JSON) with a shared trait; parallel scraping.
- Normalization to a canonical `Event` model (UTC + local times, stable ID).
- SQLite persistence + idempotent posting state.
- Post composition via configurable OpenAI‑compatible server/model.
- Manual Facebook workflow: generate drafts, copy them into Facebook, and mark shows as posted inside the app.
- Scheduler for automatic runs (interval or cron-like).
- UI for: config, venue management, "Run now", preview & approve posts, history.

---

## 2) Tech choices

- **Rust (Tauri v2)**: `reqwest`, `scraper`, `ics`, `tokio`, `rusqlite`, `serde`, `thiserror`.
- **Secrets**: `tauri-plugin-stronghold` (or OS keychain alternative). Fallback to env.
- **DB**: SQLite via `rusqlite` in Rust side.
- **Frontend**: React + TypeScript, Vite, Jotai/Zustand for state, zod for schemas.

---

## 3) Project structure

```
show-scraper/
  src/                       # React app
    app/
      routes/
        Dashboard.tsx
        Venues.tsx
        Settings.tsx
        History.tsx
      components/
        EventCard.tsx
        VenueForm.tsx
        RunPanel.tsx
      hooks/
        useConfig.ts
        useRun.ts
    lib/
      schemas.ts
      types.ts
    main.tsx
    index.css
  src-tauri/
    Cargo.toml
    tauri.conf.json
    src/
      main.rs
      db.rs
      models.rs
      normalize.rs
      llm.rs
      scheduler.rs
      scraping/
        mod.rs
        base.rs
        pine_box_html.rs
        fox_theater_ics.rs
      utils.rs
  .env.example
  README.md
```

---

## 4) Data model (Rust & TS)

### Rust `Event` (persisted as JSON)

```rust
// src-tauri/src/models.rs
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Event {
  pub id: String,                // stable hash: venue_id|start_utc|main_artist
  pub source: String,
  pub venue_id: String,
  pub venue_name: Option<String>,
  pub venue_url: Option<String>,
  pub start_local: Option<String>,
  pub start_utc: String,
  pub doors_local: Option<String>,
  pub artists: Vec<String>,
  pub is_all_ages: Option<bool>,
  pub ticket_url: Option<String>,
  pub event_url: Option<String>,
  pub price_min_cents: Option<i64>,
  pub price_max_cents: Option<i64>,
  pub currency: Option<String>,
  pub tags: Vec<String>,
  pub scraped_at_utc: String,
  pub extra: serde_json::Value,
}
```

### SQLite schema (created at startup)

```sql
CREATE TABLE IF NOT EXISTS events(
  id TEXT PRIMARY KEY,
  payload TEXT NOT NULL,
  first_seen_utc TEXT NOT NULL,
  last_seen_utc TEXT NOT NULL,
  posted_at_utc TEXT
);
CREATE TABLE IF NOT EXISTS posts(
  post_id TEXT PRIMARY KEY,
  event_id TEXT NOT NULL,
  fb_object_id TEXT,
  created_at_utc TEXT,
  status TEXT,
  response_json TEXT
);
```

### TypeScript types + zod

```ts
// src/lib/types.ts
export type Event = {
  id: string;
  source: string;
  venue_id: string;
  venue_name?: string;
  venue_url?: string;
  start_local?: string; // ISO with zone
  start_utc: string;    // ISO UTC
  doors_local?: string;
  artists: string[];
  is_all_ages?: boolean;
  ticket_url?: string;
  event_url?: string;
  price_min_cents?: number;
  price_max_cents?: number;
  currency?: string;
  tags: string[];
  scraped_at_utc: string;
  extra: Record<string, unknown>;
};

// src/lib/schemas.ts
import { z } from "zod";
export const EventSchema = z.object({
  id: z.string(),
  source: z.string(),
  venue_id: z.string(),
  venue_name: z.string().optional(),
  venue_url: z.string().url().optional(),
  start_local: z.string().optional(),
  start_utc: z.string(),
  doors_local: z.string().optional(),
  artists: z.array(z.string()),
  is_all_ages: z.boolean().optional(),
  ticket_url: z.string().url().optional(),
  event_url: z.string().url().optional(),
  price_min_cents: z.number().optional(),
  price_max_cents: z.number().optional(),
  currency: z.string().optional(),
  tags: z.array(z.string()),
  scraped_at_utc: z.string(),
  extra: z.record(z.unknown()),
});
```

---

## 5) Tauri v2 setup

### `src-tauri/Cargo.toml`

```toml
[package]
name = "show-scraper"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "1"
chrono = { version = "0.4", features = ["serde", "clock"] }
sha2 = "0.10"
base64 = "0.22"
reqwest = { version = "0.12", features = ["json", "rustls-tls"] }
scraper = "0.19"
ics = "0.5"
rusqlite = { version = "0.31", features = ["bundled", "serde_json"] }
regex = "1"
tokio = { version = "1", features = ["macros", "rt-multi-thread", "time"] }
url = "2"

[dependencies.tauri]
version = "2"
features = ["protocol-asset", "tray-icon"]

[dependencies.tauri-plugin-stronghold]
version = "2"

[build-dependencies]
tauri-build = { version = "2" }
```

### `src-tauri/tauri.conf.json` (key bits)

```json
{
  "$schema": "https://raw.githubusercontent.com/tauri-apps/tauri/v2/tooling/cli/schema.json",
  "productName": "ShowScraper",
  "version": "0.1.0",
  "identifier": "com.yourco.showscraper",
  "build": { "beforeDevCommand": "vite", "beforeBuildCommand": "vite build" },
  "app": { "windows": [{ "title": "ShowScraper", "width": 1160, "height": 760 }] },
  "security": { "csp": null }
}
```

---

## 6) Rust backend commands (IPC)

### `src-tauri/src/main.rs`

```rust
use tauri::{Manager};
mod db; mod models; mod normalize; mod llm; mod facebook; mod scheduler; mod scraping; mod utils;

#[tauri::command]
async fn run_scrape_and_post(dry_post: bool) -> Result<usize, String> {
  use crate::{db::Store, scraping::run_all, normalize::normalize_event, llm::LLMComposer, facebook::FbPoster};
  let store = Store::open_default().map_err(|e| e.to_string())?;
  let mut count = 0usize;

  // 1) scrape
  let raws = run_all().await.map_err(|e| e.to_string())?; // Vec<RawEvent>

  // 2) normalize & persist
  for raw in raws {
    let ev = normalize_event(raw)?; // -> Event
    store.upsert_event(&ev)?;
  }

  // 3) compose + post
  let composer = LLMComposer::from_env();
  let poster = FbPoster::from_vault()?; // reads token from stronghold or env
  let pending = store.pending_posts()?; // Vec<Event>

  for ev in pending {
    let msg = composer.compose(&ev).await.unwrap_or_else(|_| llm::fallback(&ev));
    if dry_post { println!("\nDRY POST:\n{}\n", msg); }
    else {
      if let Ok(fb_id) = poster.post(&msg).await { store.mark_posted(&ev.id, &fb_id)?; }
    }
    count += 1;
  }
  Ok(count)
}

pub fn run() {
  tauri::Builder::default()
    .plugin(tauri_plugin_stronghold::Builder::new("showscraper.vault").build())
    .invoke_handler(tauri::generate_handler![run_scrape_and_post])
    .run(tauri::generate_context!())
    .expect("error while running tauri app");
}

fn main(){ run(); }
```

---

## 7) Scraping module

### Common trait & types

```rust
// src-tauri/src/scraping/base.rs
use serde::{Serialize, Deserialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RawEvent {
  pub source: String,
  pub venue_id: String,
  pub venue_name: String,
  pub event_url: Option<String>,
  pub ticket_url: Option<String>,
  pub start: String,        // as-found string
  pub timezone: String,     // e.g., America/Los_Angeles
  pub artists: Vec<String>,
  pub price_text: Option<String>,
  pub extra: serde_json::Value,
}

#[async_trait::async_trait]
pub trait Scraper: Send + Sync {
  async fn fetch_events(&self) -> anyhow::Result<Vec<RawEvent>>;
}
```

### Example HTML scraper

```rust
// src-tauri/src/scraping/pine_box_html.rs
use super::base::*;
use scraper::{Html, Selector};

pub struct PineBoxScraper;

#[async_trait::async_trait]
impl Scraper for PineBoxScraper {
  async fn fetch_events(&self) -> anyhow::Result<Vec<RawEvent>> {
    let url = "https://pinebox.example.com/shows";
    let body = reqwest::get(url).await?.text().await?;
    let doc = Html::parse_document(&body);
    let card_sel = Selector::parse(".show-card").unwrap();
    let artist_sel = Selector::parse(".artist").unwrap();

    let mut items = vec![];
    for card in doc.select(&card_sel) {
      let artists: Vec<String> = card.select(&artist_sel).map(|n| n.text().collect::<String>().trim().to_string()).collect();
      let start = card.select(&Selector::parse(".date-time").unwrap()).next().map(|n| n.text().collect::<String>()).unwrap_or_default();
      let event_url = card.select(&Selector::parse("a.more").unwrap()).next().and_then(|a| a.value().attr("href")).map(|s| s.to_string());
      let ticket_url = card.select(&Selector::parse("a.ticket").unwrap()).next().and_then(|a| a.value().attr("href")).map(|s| s.to_string());
      let price_text = card.select(&Selector::parse(".price").unwrap()).next().map(|n| n.text().collect::<String>().trim().to_string());

      items.push(RawEvent{
        source: "pine_box".into(),
        venue_id: "pine_box".into(),
        venue_name: "Pine Box".into(),
        event_url, ticket_url, start,
        timezone: "America/Los_Angeles".into(),
        artists,
        price_text,
        extra: serde_json::json!({}),
      });
    }
    Ok(items)
  }
}
```

### ICS scraper

```rust
// src-tauri/src/scraping/fox_theater_ics.rs
use super::base::*;
use ics::ICalendar;

pub struct FoxTheaterICSScraper;

#[async_trait::async_trait]
impl Scraper for FoxTheaterICSScraper {
  async fn fetch_events(&self) -> anyhow::Result<Vec<RawEvent>> {
    let url = "https://foxtheater.example.com/calendar.ics";
    let text = reqwest::get(url).await?.text().await?;
    let cal: ICalendar = text.parse()?;
    let mut out = vec![];
    for ev in cal.events {
      let name = ev.summary.clone().unwrap_or_default();
      out.push(RawEvent{
        source: "fox_theater".into(),
        venue_id: "fox_theater".into(),
        venue_name: "Fox Theater".into(),
        event_url: ev.url.clone(),
        ticket_url: ev.url.clone(),
        start: ev.dtstart.as_ref().map(|d| d.value.clone()).unwrap_or_default(),
        timezone: "America/Los_Angeles".into(),
        artists: vec![name],
        price_text: None,
        extra: serde_json::json!({"location": ev.location}),
      });
    }
    Ok(out)
  }
}
```

### Runner that aggregates scrapers

```rust
// src-tauri/src/scraping/mod.rs
use super::scraping::base::{Scraper, RawEvent};
mod base; pub use base::*;
mod pine_box_html; mod fox_theater_ics;

pub async fn run_all() -> anyhow::Result<Vec<RawEvent>> {
  use futures::future::join_all;
  let scrapers: Vec<Box<dyn Scraper>> = vec![
    Box::new(pine_box_html::PineBoxScraper),
    Box::new(fox_theater_ics::FoxTheaterICSScraper),
  ];
  let futs = scrapers.into_iter().map(|s| s.fetch_events());
  let results = join_all(futs).await;
  let mut all = vec![];
  for r in results { all.extend(r?); }
  Ok(all)
}
```

---

## 8) Normalization & ID

```rust
// src-tauri/src/normalize.rs
use crate::scraping::base::RawEvent; use crate::models::Event;
use chrono::{DateTime, FixedOffset, LocalResult, TimeZone, Utc};
use sha2::{Sha256, Digest};

pub fn normalize_event(raw: RawEvent) -> anyhow::Result<Event> {
  // parse local datetime, assume timezone from raw.timezone
  let dt_local = parse_local(&raw.start, &raw.timezone)?;
  let dt_utc: DateTime<Utc> = dt_local.with_timezone(&Utc);
  let main_artist = raw.artists.get(0).cloned().unwrap_or_else(|| "TBA".into());
  let key = format!("{}|{}|{}", raw.venue_id, dt_utc.to_rfc3339(), main_artist.to_lowercase());
  let mut hasher = Sha256::new(); hasher.update(key.as_bytes());
  let id = format!("{:x}", hasher.finalize())[..16].to_string();

  Ok(Event{
    id,
    source: raw.source,
    venue_id: raw.venue_id,
    venue_name: Some(raw.venue_name),
    venue_url: None,
    start_local: Some(dt_local.to_rfc3339()),
    start_utc: dt_utc.to_rfc3339(),
    doors_local: None,
    artists: raw.artists,
    is_all_ages: None,
    ticket_url: raw.ticket_url,
    event_url: raw.event_url,
    price_min_cents: parse_price_min(raw.price_text.as_deref()),
    price_max_cents: parse_price_max(raw.price_text.as_deref()),
    currency: Some("USD".into()),
    tags: vec![],
    scraped_at_utc: Utc::now().to_rfc3339(),
    extra: raw.extra,
  })
}

fn parse_local(s: &str, tz: &str) -> anyhow::Result<DateTime<FixedOffset>> {
  // For brevity: accept RFC3339 or naive "YYYY-MM-DD HH:MM"; map tz to fixed offset  -
  // In production, prefer `chrono-tz` to resolve named zones.
  if let Ok(dt) = DateTime::parse_from_rfc3339(s) { return Ok(dt); }
  // Very naive fallback:
  let naive = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M")?;
  let offset = chrono_tz::Tz::from_str(tz).unwrap_or(chrono_tz::America::Los_Angeles);
  let local: DateTime<chrono_tz::Tz> = offset.from_local_datetime(&naive).earliest()
    .ok_or_else(|| anyhow::anyhow!("ambiguous local time"))?;
  Ok(local.with_timezone(&local.offset().fix()))
}

fn parse_price_min(s: Option<&str>) -> Option<i64> { parse_price(s).0 }
fn parse_price_max(s: Option<&str>) -> Option<i64> { parse_price(s).1 }
fn parse_price(s: Option<&str>) -> (Option<i64>, Option<i64>) {
  if let Some(txt) = s { let re = regex::Regex::new(r"\$?(\d{1,3})(?:\s*[-/–]\s*\$?(\d{1,3}))?").unwrap();
    if let Some(c) = re.captures(txt) {
      let a = c.get(1).and_then(|m| m.as_str().parse::<i64>().ok());
      let b = c.get(2).and_then(|m| m.as_str().parse::<i64>().ok());
      return (a.map(|v| v*100), b.map(|v| v*100));
    }
  } (None,None)
}
```

---

## 9) DB helper

```rust
// src-tauri/src/db.rs
use rusqlite::{Connection, params}; use crate::models::Event; use serde_json as json;
pub struct Store { conn: Connection }
impl Store {
  pub fn open_default() -> rusqlite::Result<Self> {
    let conn = Connection::open("data/shows.db")?;
    conn.execute_batch("PRAGMA journal_mode=WAL;\nCREATE TABLE IF NOT EXISTS events( id TEXT PRIMARY KEY, payload TEXT NOT NULL, first_seen_utc TEXT NOT NULL, last_seen_utc TEXT NOT NULL, posted_at_utc TEXT );\nCREATE TABLE IF NOT EXISTS posts( post_id TEXT PRIMARY KEY, event_id TEXT NOT NULL, fb_object_id TEXT, created_at_utc TEXT, status TEXT, response_json TEXT );")?;
    Ok(Self{conn})
  }
  pub fn upsert_event(&self, ev: &Event) -> rusqlite::Result<()> {
    let now = &ev.scraped_at_utc; let payload = json::to_string(ev).unwrap();
    let exists: Option<i64> = self.conn.query_row("SELECT 1 FROM events WHERE id=?1", params![&ev.id], |r| r.get(0)).optional()?;
    if exists.is_some() { self.conn.execute("UPDATE events SET payload=?1, last_seen_utc=?2 WHERE id=?3", params![payload, now, &ev.id])?; }
    else { self.conn.execute("INSERT INTO events(id,payload,first_seen_utc,last_seen_utc,posted_at_utc) VALUES(?1,?2,?3,?3,NULL)", params![&ev.id, payload, now])?; }
    Ok(())
  }
  pub fn pending_posts(&self) -> rusqlite::Result<Vec<Event>> {
    let mut stmt = self.conn.prepare("SELECT payload FROM events WHERE posted_at_utc IS NULL AND json_extract(payload,'$.start_utc') >= datetime('now')")?;
    let rows = stmt.query_map([], |row| {
      let s: String = row.get(0)?; Ok(serde_json::from_str::<Event>(&s).unwrap())
    })?; Ok(rows.filter_map(Result::ok).collect())
  }
  pub fn mark_posted(&self, event_id: &str, fb_object_id: &str) -> rusqlite::Result<()> {
    self.conn.execute("UPDATE events SET posted_at_utc=datetime('now') WHERE id=?1", params![event_id])?;
    self.conn.execute("INSERT OR REPLACE INTO posts(post_id,event_id,fb_object_id,created_at_utc,status,response_json) VALUES(?1,?1,?2,datetime('now'),'ok',NULL)", params![event_id, fb_object_id])?; Ok(())
  }
}
```

---

## 10) LLM post composer (OpenAI‑compatible)

````rust
// src-tauri/src/llm.rs
use serde::{Serialize, Deserialize}; use reqwest::Client; use crate::models::Event;

#[derive(Clone)]
pub struct LLMComposer { base_url: String, api_key: Option<String>, model: String, temperature: f32, max_tokens: u32, max_chars: usize, style: String, http: Client }

impl LLMComposer {
  pub fn from_env() -> Self {
    let base_url = std::env::var("LLM_BASE_URL").unwrap_or_else(|_| "https://api.openai.com/v1".into());
    let model = std::env::var("LLM_MODEL").unwrap_or_else(|_| "gpt-4o-mini".into());
    let temperature = std::env::var("LLM_TEMPERATURE").ok().and_then(|s| s.parse().ok()).unwrap_or(0.2);
    let max_tokens = std::env::var("LLM_MAX_TOKENS").ok().and_then(|s| s.parse().ok()).unwrap_or(350);
    let max_chars = std::env::var("LLM_MAX_CHARS").ok().and_then(|s| s.parse().ok()).unwrap_or(900usize);
    let style = std::env::var("LLM_STYLE").unwrap_or_else(|_| "concise".into());
    let api_key = std::env::var("LLM_API_KEY").ok();
    Self{ base_url, api_key, model, temperature, max_tokens, max_chars, style, http: Client::new() }
  }

  pub async fn compose(&self, ev: &Event) -> anyhow::Result<String> {
    let payload = serde_json::json!({
      "artists": ev.artists,
      "venue_name": ev.venue_name,
      "start_local": ev.start_local,
      "ticket_url": ev.ticket_url,
      "event_url": ev.event_url,
      "price_min_cents": ev.price_min_cents,
      "price_max_cents": ev.price_max_cents,
      "currency": ev.currency,
      "address": ev.extra.get("address"),
      "tags": ev.tags,
    });

    let sys = DEFAULT_SYSTEM;
    let user = format!(USER_PROMPT, max_chars=self.max_chars, style=self.style, json=serde_json::to_string_pretty(&payload).unwrap());

    let req = serde_json::json!({
      "model": self.model,
      "temperature": self.temperature,
      "max_tokens": self.max_tokens,
      "messages": [
        {"role":"system","content": sys},
        {"role":"user","content": user}
      ]
    });

    let mut b = self.http.post(format!("{}/chat/completions", self.base_url)).json(&req);
    if let Some(key) = &self.api_key { b = b.bearer_auth(key); }
    let resp = b.send().await?.error_for_status()?;
    let v: serde_json::Value = resp.json().await?;
    let text = v["choices"][0]["message"]["content"].as_str().unwrap_or("").to_string();
    let out = if text.len() > self.max_chars { text[..self.max_chars].to_string() } else { text };
    Ok(out)
  }
}

const DEFAULT_SYSTEM: &str = "You are a copywriter for Facebook GROUP posts. Use only provided JSON fields. Concise. No invented facts. Include ticket link if provided. American English.";
const USER_PROMPT: &str = r#"Format a short Facebook group post for this show:\n\nJSON:\n```\n{json}\n```\nRules:\n- Line 1: headliner (artists[0]) and venue if available.\n- Line 2: human-readable local date/time from start_local.\n- Include ticket link if present.\n- If price_min_cents/price_max_cents exist, show a simple range in currency.\n- Keep under {max_chars} characters.\n- No made-up info.\n- Style: {style}.\nOutput ONLY the post text."#;

pub fn fallback(ev: &Event) -> String {
  let main = ev.artists.get(0).cloned().unwrap_or_else(|| "TBA".into());
  let mut s = format!("{}", main);
  if let Some(v) = &ev.venue_name { s.push_str(&format!(" — {}", v)); }
  if let Some(dt) = &ev.start_local { s.push_str(&format!("\n{}", dt)); }
  if let Some(t) = &ev.ticket_url { s.push_str(&format!("\n🎟 Tickets: {}", t)); }
  if let Some(u) = &ev.event_url { s.push_str(&format!("\nℹ️ Details: {}", u)); }
  s
}
````

---

## 11) Facebook publisher

```rust
// src-tauri/src/facebook.rs
use reqwest::Client; use serde_json as json; use anyhow::Result;

pub struct FbPoster { group_id: String, token: String, http: Client }
impl FbPoster {
  pub fn from_vault() -> Result<Self> {
    // Read from stronghold or env var FB_ACCESS_TOKEN
    let token = std::env::var("FB_ACCESS_TOKEN").map_err(|_| anyhow::anyhow!("Missing FB_ACCESS_TOKEN"))?;
    let group_id = std::env::var("FB_GROUP_ID").map_err(|_| anyhow::anyhow!("Missing FB_GROUP_ID"))?;
    Ok(Self{ group_id, token, http: Client::new() })
  }
  pub async fn post(&self, message: &str) -> Result<String> {
    let url = format!("https://graph.facebook.com/v21.0/{}/feed", self.group_id);
    let resp = self.http.post(url)
      .form(&[ ("message", message), ("access_token", &self.token) ])
      .send().await?.error_for_status()?;
    let v: serde_json::Value = resp.json().await?;
    Ok(v["id"].as_str().unwrap_or("").to_string())
  }
}
```

---

## 12) Scheduler (optional background)

```rust
// src-tauri/src/scheduler.rs
use tokio::time::{interval, Duration}; use tauri::AppHandle;

pub fn spawn(app: &AppHandle){
  let handle = app.clone();
  tauri::async_runtime::spawn(async move {
    let mut tick = interval(Duration::from_secs(60*30)); // every 30 minutes
    loop {
      tick.tick().await;
      if let Err(e) = super::run_scrape_and_post(false).await { eprintln!("scheduler error: {}", e); }
    }
  });
}
```

---

## 13) Frontend – minimal pieces

### Invoke from React (run now)

```ts
// src/app/components/RunPanel.tsx
import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export default function RunPanel(){
  const [busy, setBusy] = useState(false);
  const [out, setOut] = useState<string | null>(null);
  return (
    <div className="p-4 rounded-xl border">
      <button disabled={busy} onClick={async()=>{
        setBusy(true);
        try { const n = await invoke<number>("run_scrape_and_post", { dryPost: true }); setOut(`Processed ${n} events (dry run).`); }
        catch(e){ setOut(String(e)); }
        finally{ setBusy(false); }
      }} className="px-4 py-2 rounded-lg border">Run (Dry)</button>
      {out && <pre className="mt-3 whitespace-pre-wrap text-sm">{out}</pre>}
    </div>
  );
}
```

### Settings screen (env-backed)

- Inputs for `FB_GROUP_ID`, `FB_ACCESS_TOKEN`, `LLM_BASE_URL`, `LLM_MODEL`, `LLM_API_KEY`, etc.
- Save to OS keychain/stronghold via a small Rust command (not shown) or instruct users to set env before launch.

---

## 14) Configuration & secrets

Environment variables (recommended):

```
FB_GROUP_ID=1234567890
FB_ACCESS_TOKEN=EAAG-... (long-lived)
LLM_BASE_URL=https://api.openai.com/v1
LLM_MODEL=gpt-4o-mini
LLM_API_KEY=sk-...
LLM_TEMPERATURE=0
LLM_MAX_TOKENS=350
LLM_MAX_CHARS=900
```

For local LLMs: set `LLM_BASE_URL=http://localhost:8000/v1` and omit `LLM_API_KEY` if your server doesn’t require it.

---

## 15) JS-heavy venue pages (options)

- Prefer ICS/JSON feeds when available.
- If the site is JS-only, add a **sidecar** service (e.g., Playwright service) and fetch via HTTP from Rust.
- Or use cached network calls to the site’s underlying JSON endpoints (inspect dev tools). Avoid brittle DOM scraping where possible.

---

## 16) Testing strategy

- Rust unit tests for: price parsing, ID determinism, normalization.
- Golden HTML/ICS fixtures for scrapers.
- Dry-run E2E: invoke `run_scrape_and_post(dry_post=true)` and snapshot the composed text.

---

## 17) Next steps

1. Wire up two real venue adapters you care about (send URLs/selectors; we’ll fill them in).
2. Hook up the settings page to store/read env (or stronghold vault).
3. Add a preview list of **pending events** with a “Post now” button.

---

*This blueprint gives you a working spine: IPC commands, data model, scraping, LLM composition, Facebook posting, and React entry points—ready to tailor to your exact venue list.*



---

# Manual Posting Queue — Spec & Code

> **Change of behavior:** No automatic scheduling or auto-posting. Users must manually trigger posting. The app shows a **Pending Posts** page that groups upcoming shows by proximity: **< 2 Months**, **< 1 Month**, **< 2 Weeks**, **< 1 Week**, **Day Of**.

## Product requirements

- A **Pending Posts** page lists all unposted future events.
- Each row shows: headliner, venue, local datetime, ticket link badge if present, and a **Preview** action to fetch LLM-composed text.
- Groups (buckets) by time-to-show relative to *now*:
  - `DAY_OF` (0 days)
  - `< 1 WEEK` (1–6 days)
  - `< 2 WEEKS` (7–13 days)
  - `< 1 MONTH` (14–29 days)
  - `< 2 MONTHS` (30–59 days)
  - `≥ 2 MONTHS` (60+ days) — optional collapsed group
- Select one or many events and click **Post to Facebook** (manual only).
- After successful post, the item disappears from Pending and moves to **History**.
- Provide **Dry Run** toggle to render the post text without publishing.

## Backend: DB query and bucketing

### SQL (events still unposted, future-dated)

```sql
-- Uses SQLite date math (UTC). Keep in Rust but shown here for clarity.
SELECT 
  payload,
  CAST(ROUND(julianday(json_extract(payload,'$.start_utc')) - julianday('now')) AS INTEGER) AS days_until
FROM events
WHERE posted_at_utc IS NULL
  AND json_extract(payload,'$.start_utc') >= datetime('now')
ORDER BY json_extract(payload,'$.start_utc') ASC;
```

### Rust: bucketing + new commands

```rust
// src-tauri/src/db.rs (add helper type)
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct PendingRow { pub event: crate::models::Event, pub days_until: i32 }

impl Store {
  pub fn pending_with_days(&self) -> rusqlite::Result<Vec<PendingRow>> {
    let mut stmt = self.conn.prepare(
      "SELECT payload, CAST(ROUND(julianday(json_extract(payload,'$.start_utc')) - julianday('now')) AS INTEGER) AS d FROM events WHERE posted_at_utc IS NULL AND json_extract(payload,'$.start_utc') >= datetime('now') ORDER BY json_extract(payload,'$.start_utc') ASC"
    )?;
    let rows = stmt.query_map([], |row| {
      let payload: String = row.get(0)?;
      let d: i32 = row.get(1)?;
      let ev: crate::models::Event = serde_json::from_str(&payload).unwrap();
      Ok(PendingRow{ event: ev, days_until: d })
    })?;
    Ok(rows.filter_map(Result::ok).collect())
  }
}

// src-tauri/src/llm.rs (add public compose_preview)
impl LLMComposer {
  pub async fn compose_preview(&self, ev: &crate::models::Event) -> anyhow::Result<String> { self.compose(ev).await }
}

// src-tauri/src/facebook.rs (add single-event post wrapper already available)
```

```rust
// src-tauri/src/main.rs (new IPC commands)
#[tauri::command]
async fn list_pending_buckets() -> Result<serde_json::Value, String> {
  use crate::db::Store;
  let store = Store::open_default().map_err(|e| e.to_string())?;
  let rows = store.pending_with_days().map_err(|e| e.to_string())?;
  fn bucket(d: i32) -> &'static str {
    if d <= 0 { "DAY_OF" }
    else if d < 7 { "LT_1W" }
    else if d < 14 { "LT_2W" }
    else if d < 30 { "LT_1M" }
    else if d < 60 { "LT_2M" }
    else { "GTE_2M" }
  }
  let mut map: std::collections::BTreeMap<&'static str, Vec<serde_json::Value>> = Default::default();
  for r in rows {
    map.entry(bucket(r.days_until)).or_default().push(serde_json::json!({
      "days_until": r.days_until,
      "event": r.event,
    }));
  }
  Ok(serde_json::to_value(map).unwrap())
}

#[tauri::command]
async fn preview_post(event_id: String) -> Result<String, String> {
  use crate::{db::Store, llm::LLMComposer, llm};
  let store = Store::open_default().map_err(|e| e.to_string())?;
  // Load single event by id
  let ev = store.get_event(&event_id).map_err(|e| e.to_string())?; // implement get_event
  let comp = LLMComposer::from_env();
  match comp.compose_preview(&ev).await { Ok(s) => Ok(s), Err(_) => Ok(llm::fallback(&ev)) }
}

#[tauri::command]
async fn post_to_facebook(event_id: String) -> Result<String, String> {
  use crate::{db::Store, facebook::FbPoster, llm::LLMComposer, llm};
  let store = Store::open_default().map_err(|e| e.to_string())?;
  let ev = store.get_event(&event_id).map_err(|e| e.to_string())?;
  let msg = LLMComposer::from_env().compose(&ev).await.unwrap_or_else(|_| llm::fallback(&ev));
  let fb = FbPoster::from_vault().map_err(|e| e.to_string())?;
  let fb_id = fb.post(&msg).await.map_err(|e| e.to_string())?;
  store.mark_posted(&event_id, &fb_id).map_err(|e| e.to_string())?;
  Ok(fb_id)
}
```

```rust
// src-tauri/src/db.rs (add get_event)
impl Store {
  pub fn get_event(&self, id: &str) -> rusqlite::Result<crate::models::Event> {
    let s: String = self.conn.query_row("SELECT payload FROM events WHERE id=?1", rusqlite::params![id], |r| r.get(0))?;
    Ok(serde_json::from_str(&s).unwrap())
  }
}
```

> Remove/disable the scheduler: do **not** spawn periodic jobs. Only the UI triggers `post_to_facebook`.

## Frontend: Pending Posts page

### Route and component

```tsx
// src/app/routes/PendingPosts.tsx
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Event } from "../../lib/types";

type BucketKey = "DAY_OF"|"LT_1W"|"LT_2W"|"LT_1M"|"LT_2M"|"GTE_2M";
const LABELS: Record<BucketKey,string> = { DAY_OF:"Day of", LT_1W:"< 1 Week", LT_2W:"< 2 Weeks", LT_1M:"< 1 Month", LT_2M:"< 2 Months", GTE_2M:"≥ 2 Months" };

export default function PendingPosts(){
  const [data, setData] = useState<Record<BucketKey, {days_until:number; event:Event}[]>>({} as any);
  const [selected, setSelected] = useState<Record<string, boolean>>({});
  const [preview, setPreview] = useState<string>("");
  const [busy, setBusy] = useState(false);

  async function refresh(){
    const res = await invoke<Record<BucketKey, any>>("list_pending_buckets");
    setData(res as any);
  }
  useEffect(()=>{ refresh(); }, []);

  const ids = Object.keys(selected).filter(k=>selected[k]);

  return (
    <div className="p-6 space-y-6">
      <div className="flex items-center gap-3">
        <button className="border rounded px-3 py-2" onClick={refresh}>Refresh</button>
        <button disabled={!ids.length || busy} className="border rounded px-3 py-2" onClick={async()=>{
          setBusy(true);
          try{
            for(const id of ids){ await invoke<string>("post_to_facebook", { eventId: id }); }
            setSelected({}); await refresh();
          } finally { setBusy(false); }
        }}>Post to Facebook ({ids.length})</button>
      </div>

      {(Object.keys(LABELS) as BucketKey[]).map(key => {
        const rows = (data?.[key]||[]) as {days_until:number; event:Event}[];
        if (!rows.length) return null;
        return (
          <section key={key} className="space-y-2">
            <h2 className="text-lg font-semibold mt-6">{LABELS[key]} <span className="text-sm opacity-70">({rows.length})</span></h2>
            <div className="grid gap-2">
              {rows.map(({days_until, event})=> (
                <div key={event.id} className="border rounded-xl p-3 flex items-center gap-3">
                  <input type="checkbox" checked={!!selected[event.id]} onChange={e=>setSelected(s=>({...s,[event.id]:e.target.checked}))} />
                  <div className="flex-1">
                    <div className="font-medium">{event.artists?.[0]||"TBA"} — {event.venue_name||""}</div>
                    <div className="text-sm opacity-80">{new Date(event.start_local||event.start_utc).toLocaleString()}</div>
                  </div>
                  {event.ticket_url && <a className="text-sm underline" href={event.ticket_url} target="_blank">Tickets</a>}
                  <button className="text-sm border rounded px-2 py-1" onClick={async()=>{
                    const txt = await invoke<string>("preview_post", { eventId: event.id });
                    setPreview(txt);
                  }}>Preview</button>
                </div>
              ))}
            </div>
          </section>
        );
      })}

      {preview && (
        <div className="border rounded-xl p-4 bg-neutral-50">
          <div className="text-sm opacity-70 mb-2">Post Preview</div>
          <pre className="whitespace-pre-wrap text-sm">{preview}</pre>
        </div>
      )}
    </div>
  );
}
```

### Navigation

- Add a sidebar link **Pending Posts** → `/pending` (register route and include the component).

## UX details

- Default sort: soonest first within each bucket.
- Multi-select persists across bucket refresh until posted.
- Disabled “Post” button while posting; show toast on success/failure.
- Absolutely no background posting; the only pathway is the button.

## QA checklist

-

