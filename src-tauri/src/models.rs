use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Event {
    pub id: String, // stable hash: venue_id|start_utc|main_artist
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

impl Event {
    pub fn title(&self) -> String {
        self.artists
            .first()
            .cloned()
            .unwrap_or_else(|| "Untitled Event".to_string())
    }
}
