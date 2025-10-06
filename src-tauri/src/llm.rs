use chrono::{DateTime, Local};
use reqwest::Client;
use serde_json::json;
use thiserror::Error;

use crate::models::Event;

#[allow(dead_code)]
#[derive(Debug, Error)]
pub enum ComposeError {
    #[error("composer unavailable: {0}")]
    Unavailable(String),
}

const DEFAULT_ENDPOINT: &str = "http://127.0.0.1:1234/v1";
const DEFAULT_MODEL: &str = "gpt-4o-mini";
const DEFAULT_TEMPERATURE: f32 = 0.2;
const DEFAULT_MAX_TOKENS: u32 = 5000;
const DEFAULT_STYLE: &str = "concise";

pub struct LLMComposer {
    model: String,
    base_url: String,
    api_key: Option<String>,
    temperature: f32,
    max_tokens: u32,
    style: String,
    client: Client,
}

impl LLMComposer {
    pub fn from_env() -> Self {
        let base_url =
            std::env::var("LLM_ENDPOINT").unwrap_or_else(|_| DEFAULT_ENDPOINT.to_string());
        let model = std::env::var("LLM_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string());
        let api_key = std::env::var("LLM_API_KEY").ok();
        let temperature = std::env::var("LLM_TEMPERATURE")
            .ok()
            .and_then(|s| s.parse::<f32>().ok())
            .unwrap_or(DEFAULT_TEMPERATURE);
        let max_tokens = std::env::var("LLM_MAX_TOKENS")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(DEFAULT_MAX_TOKENS);
        let style = std::env::var("LLM_STYLE").unwrap_or_else(|_| DEFAULT_STYLE.to_string());

        Self {
            model,
            base_url,
            api_key,
            temperature,
            max_tokens,
            style,
            client: Client::new(),
        }
    }

    pub async fn compose_preview(&self, event: &Event) -> Result<String, ComposeError> {
        self.compose_internal(event, true).await
    }

    pub async fn compose(&self, event: &Event) -> Result<String, ComposeError> {
        self.compose_internal(event, false).await
    }
}

pub fn fallback(event: &Event) -> String {
    render_post(event)
}

pub fn fallback_preview(event: &Event) -> String {
    render_preview(event)
}

impl LLMComposer {
    async fn compose_internal(&self, event: &Event, preview: bool) -> Result<String, ComposeError> {
        let base = self.base_url.trim_end_matches('/');
        let url = format!("{}/chat/completions", base);

        let context = if preview {
            "internal preview"
        } else {
            "Facebook group"
        };

        let event_json = serde_json::to_string_pretty(&event_payload(event)).unwrap_or_default();
        let payload = json!({
            "model": self.model,
            "temperature": self.temperature,
            "max_tokens": self.max_tokens,
            "messages": [
                {
                    "role": "system",
                    "content": default_system(preview),
                },
                {
                    "role": "user",
                    "content": build_user_prompt(context, &self.style, &event_json),
                }
            ],
        });

        let mut request = self.client.post(url).json(&payload);
        if let Some(key) = &self.api_key {
            request = request.bearer_auth(key);
        }

        let response = request
            .send()
            .await
            .map_err(|err| ComposeError::Unavailable(err.to_string()))?;

        let status = response.status();
        let text_body = response
            .text()
            .await
            .map_err(|err| ComposeError::Unavailable(err.to_string()))?;

        if !status.is_success() {
            return Err(ComposeError::Unavailable(format!(
                "HTTP {}: {}",
                status, text_body
            )));
        }

        let value: serde_json::Value = serde_json::from_str(&text_body)
            .map_err(|err| ComposeError::Unavailable(err.to_string()))?;

        let text = value
            .get("choices")
            .and_then(|choices| choices.as_array())
            .and_then(|choices| choices.first())
            .and_then(|choice| choice.get("message"))
            .and_then(|message| message.get("content"))
            .and_then(|content| content.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| ComposeError::Unavailable("LLM response missing content".to_string()))?;

        Ok(text)
    }
}

fn default_system(preview: bool) -> &'static str {
    if preview {
        "You summarize upcoming shows for internal review. Keep it concise and factual."
    } else {
        "You’re an electronic music die-hard hyping shows to fellow ravers in a Facebook GROUP. Speak like a trusted friend in the scene: energetic, slang-savvy, and respectful. Use only provided info. Spotlight the headliner, call out the vibe/genre, and make it sound like a can’t-miss night. Always include ticket and event links when available. No made-up details. American English."
    }
}

fn event_payload(event: &Event) -> serde_json::Value {
    json!({
        "artists": event.artists,
        "venue_name": event.venue_name,
        "start_local": event.start_local,
        "start_utc": event.start_utc,
        "ticket_url": event.ticket_url,
        "event_url": event.event_url,
        "price_min_cents": event.price_min_cents,
        "price_max_cents": event.price_max_cents,
        "currency": event.currency,
        "tags": event.tags,
        "extra": event.extra,
    })
}

fn build_user_prompt(context: &str, style: &str, event_json: &str) -> String {
    format!(
        "Format a short {context} post for this show.\n\nJSON DATA:\n{json}\n\nRules:\n- Style: {style}.\n- Sound like one raver hyping another.\n- Hook readers with the headliner and venue immediately.\n- Describe the music vibe/genre using provided tags or notes (skip if unavailable).\n- Include ticket and event links when present.\n- Keep it punchy, high-energy, and authentic to the electronic scene.\n",
        context = context,
        style = style,
        json = event_json
    )
}

fn render_preview(event: &Event) -> String {
    let local_time = parse_time(event).map(|dt| dt.format("%a %b %e @ %l:%M %p").to_string());
    format!(
        "{title}\nVenue: {venue}\nWhen: {when}\nTickets: {tickets}",
        title = event.title(),
        venue = event
            .venue_name
            .clone()
            .unwrap_or_else(|| "Unknown Venue".to_string()),
        when = local_time.unwrap_or_else(|| event.start_utc.clone()),
        tickets = event
            .ticket_url
            .clone()
            .unwrap_or_else(|| "TBA".to_string()),
    )
}

fn render_post(event: &Event) -> String {
    let local_time = parse_time(event)
        .map(|dt| dt.format("%A, %B %e at %l:%M %p").to_string())
        .unwrap_or_else(|| event.start_utc.clone());
    let vibe = event
        .tags
        .first()
        .cloned()
        .filter(|tag| !tag.trim().is_empty())
        .map(|tag| format!("Sound: {tag}"));

    let mut lines = vec![
        event.title(),
        event
            .venue_name
            .clone()
            .unwrap_or_else(|| "Unknown Venue".to_string()),
        local_time,
    ];

    if let Some(vibe_line) = vibe {
        lines.push(vibe_line);
    }

    if let Some(ticket) = &event.ticket_url {
        lines.push(format!("🎟 Tickets: {ticket}"));
    }

    if let Some(details) = &event.event_url {
        lines.push(format!("ℹ️ Event: {details}"));
    }

    if !lines.iter().any(|line| line.contains("🎟")) {
        lines.push("🎟 Tickets: TBA".to_string());
    }

    lines.join("\n")
}

fn parse_time(event: &Event) -> Option<DateTime<Local>> {
    event
        .start_local
        .as_ref()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .or_else(|| DateTime::parse_from_rfc3339(&event.start_utc).ok())
        .map(|dt| dt.with_timezone(&Local))
}
