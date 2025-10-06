use std::collections::HashMap;
use std::sync::Mutex;

use once_cell::sync::Lazy;
use reqwest::{Client, Url};
use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::models::Event;

static CLIENT: Lazy<Client> = Lazy::new(|| {
    let user_agent = std::env::var("MUSICBRAINZ_USER_AGENT")
        .unwrap_or_else(|_| "show-scrape/0.1 (https://github.com/mike/show-scrape)".to_string());
    Client::builder()
        .user_agent(user_agent)
        .build()
        .expect("failed to build musicbrainz client")
});

static CACHE: Lazy<Mutex<HashMap<String, Option<ArtistProfile>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

#[derive(Debug, Clone, Deserialize)]
pub struct ArtistProfile {
    pub id: String,
    pub name: String,
    pub disambiguation: Option<String>,
    pub genres: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum MusicBrainzError {
    #[error("http error: {0}")]
    Http(String),
    #[error("parse error: {0}")]
    Parse(String),
}

#[derive(Debug, Deserialize)]
struct ArtistSearchResponse {
    artists: Option<Vec<ArtistDoc>>,
}

#[derive(Debug, Deserialize)]
struct ArtistDoc {
    id: String,
    name: String,
    disambiguation: Option<String>,
    #[serde(default)]
    tags: Vec<TagDoc>,
    #[serde(default)]
    genres: Vec<TagDoc>,
}

#[derive(Debug, Deserialize)]
struct TagDoc {
    name: String,
}

pub async fn enrich_event(mut event: Event) -> Result<Event, MusicBrainzError> {
    let artist_name = match event.artists.first() {
        Some(name) if !name.trim().is_empty() => name.trim(),
        _ => return Ok(event),
    };

    let profile = lookup_artist(artist_name).await?;
    if let Some(profile) = profile {
        let mut genres: Vec<String> = event.tags.clone();
        for genre in &profile.genres {
            if !genres
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(genre))
            {
                genres.push(genre.clone());
            }
        }
        event.tags = genres;

        let mut extra_map = match event.extra {
            Value::Object(map) => map,
            _ => Map::new(),
        };
        extra_map.insert(
            "musicbrainz".to_string(),
            json!({
                "id": profile.id,
                "name": profile.name,
                "disambiguation": profile.disambiguation,
                "genres": profile.genres,
            }),
        );
        event.extra = Value::Object(extra_map);
    }

    Ok(event)
}

async fn lookup_artist(name: &str) -> Result<Option<ArtistProfile>, MusicBrainzError> {
    let key = name.to_lowercase();
    let cached_opt = {
        let guard = CACHE.lock().expect("musicbrainz cache poisoned");
        guard.get(&key).cloned()
    };
    if let Some(cached) = cached_opt {
        return Ok(cached);
    }

    let sanitized = name.replace('"', " ");
    let mut url = Url::parse("https://musicbrainz.org/ws/2/artist/")
        .map_err(|err| MusicBrainzError::Http(err.to_string()))?;
    url.query_pairs_mut()
        .append_pair("query", &format!("artist:\"{}\"", sanitized))
        .append_pair("fmt", "json")
        .append_pair("limit", "1")
        .append_pair("inc", "tags+genres");

    let response = CLIENT
        .get(url)
        .send()
        .await
        .map_err(|err| MusicBrainzError::Http(err.to_string()))?;
    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|err| MusicBrainzError::Http(err.to_string()))?;

    if !status.is_success() {
        return Err(MusicBrainzError::Http(format!(
            "status {}: {}",
            status, text
        )));
    }

    let payload: ArtistSearchResponse =
        serde_json::from_str(&text).map_err(|err| MusicBrainzError::Parse(err.to_string()))?;

    let profile = payload
        .artists
        .and_then(|mut list| list.pop())
        .map(|artist| {
            let genres = extract_genres(&artist);
            ArtistProfile {
                id: artist.id,
                name: artist.name,
                disambiguation: artist.disambiguation,
                genres,
            }
        })
        .filter(|profile| !profile.genres.is_empty());

    CACHE
        .lock()
        .expect("musicbrainz cache poisoned")
        .insert(key, profile.clone());

    Ok(profile)
}

fn extract_genres(doc: &ArtistDoc) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for tag in doc.genres.iter().chain(doc.tags.iter()) {
        let clean = tag.name.trim();
        if clean.is_empty() {
            continue;
        }
        if !out
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(clean))
        {
            out.push(clean.to_string());
        }
    }
    out
}
