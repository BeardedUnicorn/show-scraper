use chrono::{DateTime, Duration, Local, Utc};
use rusqlite::{params, Connection};
use serde_json::json;

use crate::models::Event;
use crate::utils;

pub struct Store {
    conn: Connection,
}

pub struct PendingEvent {
    pub event: Event,
}

impl Store {
    pub fn open_default() -> rusqlite::Result<Self> {
        let path = utils::database_path();
        utils::ensure_parent(&path);
        let conn = Connection::open(path)?;
        let store = Self { conn };
        store.init_schema()?;
        store.seed_if_empty()?;
        Ok(store)
    }

    fn init_schema(&self) -> rusqlite::Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS events(
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
            );",
        )?;
        Ok(())
    }

    fn seed_if_empty(&self) -> rusqlite::Result<()> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))?;
        if count > 0 {
            return Ok(());
        }

        let now = Utc::now();
        let samples = vec![
            sample_event("pine_box", "Pine Box Rock Shop", now + Duration::days(1)),
            sample_event("venus", "Venus Lounge", now + Duration::days(6)),
            sample_event("fox_theater", "The Fox Theater", now + Duration::days(14)),
        ];

        for event in samples {
            self.upsert_event(&event)?;
        }

        Ok(())
    }

    pub fn upsert_event(&self, event: &Event) -> rusqlite::Result<()> {
        let now = Utc::now().to_rfc3339();
        let payload = serde_json::to_string(event).expect("event serialization");
        self.conn.execute(
            "INSERT INTO events (id, payload, first_seen_utc, last_seen_utc, posted_at_utc)
             VALUES (?1, ?2, ?3, ?3, NULL)
             ON CONFLICT(id) DO UPDATE SET
               payload = excluded.payload,
               last_seen_utc = excluded.last_seen_utc",
            params![event.id, payload, now],
        )?;
        Ok(())
    }

    pub fn list_pending_events(&self) -> rusqlite::Result<Vec<PendingEvent>> {
        let mut stmt = self
            .conn
            .prepare("SELECT payload FROM events WHERE posted_at_utc IS NULL")?;
        let rows = stmt.query_map([], |row| {
            let payload: String = row.get(0)?;
            let event: Event = serde_json::from_str(&payload).map_err(|err| {
                rusqlite::Error::FromSqlConversionFailure(
                    payload.len(),
                    rusqlite::types::Type::Text,
                    Box::new(err),
                )
            })?;
            Ok(PendingEvent { event })
        })?;

        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn get_event(&self, id: &str) -> rusqlite::Result<Event> {
        let payload: String = self.conn.query_row(
            "SELECT payload FROM events WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )?;
        let event: Event = serde_json::from_str(&payload).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(
                payload.len(),
                rusqlite::types::Type::Text,
                Box::new(err),
            )
        })?;
        Ok(event)
    }

    pub fn mark_posted(&self, event_id: &str, fb_id: &str) -> rusqlite::Result<()> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "UPDATE events SET posted_at_utc = ?2, last_seen_utc = ?2 WHERE id = ?1",
            params![event_id, now],
        )?;
        let post_payload = json!({
            "fb_id": fb_id,
            "posted_at": now,
        })
        .to_string();
        self.conn.execute(
            "INSERT INTO posts (post_id, event_id, fb_object_id, created_at_utc, status, response_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                fb_id,
                event_id,
                fb_id,
                now,
                "posted",
                post_payload
            ],
        )?;
        Ok(())
    }
}

fn sample_event(venue_id: &str, venue_name: &str, start: DateTime<Utc>) -> Event {
    let start_iso = start.to_rfc3339();
    Event {
        id: format!("{venue_id}|{start_iso}|headliner"),
        source: "seed".to_string(),
        venue_id: venue_id.to_string(),
        venue_name: Some(venue_name.to_string()),
        venue_url: None,
        start_local: Some(start.with_timezone(&Local).to_rfc3339()),
        start_utc: start_iso.clone(),
        doors_local: None,
        artists: vec!["Sample Artist".to_string()],
        is_all_ages: Some(true),
        ticket_url: Some("https://tickets.example.com".to_string()),
        event_url: Some("https://events.example.com".to_string()),
        price_min_cents: Some(1500),
        price_max_cents: Some(2500),
        currency: Some("USD".to_string()),
        tags: vec!["Rock".to_string()],
        scraped_at_utc: Utc::now().to_rfc3339(),
        extra: json!({}),
    }
}
