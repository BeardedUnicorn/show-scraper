use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use chrono::{
    DateTime, Datelike, Local, LocalResult, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc,
};
use chrono_tz::Tz;
use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::blocking::Client;
use scraper::{ElementRef, Selector};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::models::Event;

static TIME_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)(\d{1,2})(?::(\d{2}))?\s*(am|pm)").expect("valid time regex"));

pub fn clean_text(input: &str) -> String {
    input
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

pub fn first_text(element: &ElementRef<'_>, selector: &Selector) -> Option<String> {
    element
        .select(selector)
        .next()
        .map(|node| {
            let text = inner_text(node);
            let cleaned = clean_text(&text);
            if cleaned.is_empty() {
                None
            } else {
                Some(cleaned)
            }
        })
        .flatten()
}

pub fn inner_text(element: ElementRef<'_>) -> String {
    clean_text(&element.text().collect::<Vec<_>>().join(" "))
}

pub fn first_attr(element: &ElementRef<'_>, selector: &Selector, attr: &str) -> Option<String> {
    element
        .select(selector)
        .next()
        .and_then(|el| el.value().attr(attr))
        .map(str::to_string)
}

pub fn absolute_url(base: &str, href: Option<String>) -> Option<String> {
    let href = href?;
    if href.starts_with("http://") || href.starts_with("https://") {
        return Some(href);
    }
    let base_url = reqwest::Url::parse(base).ok()?;
    base_url.join(&href).ok().map(|u| u.to_string())
}

pub fn fetch_html(url: &str) -> Result<String> {
    static CLIENT: Lazy<Client> = Lazy::new(|| {
        Client::builder()
            .timeout(Duration::from_secs(20))
            .user_agent("ShowScraper/0.1 (+https://github.com/mike/show-scrape)")
            .build()
            .expect("http client")
    });

    let response = CLIENT
        .get(url)
        .send()
        .with_context(|| format!("request failed for {url}"))?;
    let response = response
        .error_for_status()
        .with_context(|| format!("non-success status for {url}"))?;
    response
        .text()
        .with_context(|| format!("unable to read response body for {url}"))
}

pub fn split_artists(text: &str) -> Vec<String> {
    if text.trim().is_empty() {
        return Vec::new();
    }
    let mut normalized = text.to_string();
    for ch in [',', '/', '&', '+'] {
        normalized = normalized.replace(ch, ",");
    }
    normalized = normalized.replace(" w/", ",");
    normalized = normalized.replace(" with ", ",");
    normalized = normalized.replace(" With ", ",");
    normalized = normalized.replace(" feat. ", ",");
    normalized = normalized.replace(" ft. ", ",");
    normalized = normalized.replace(" featuring ", ",");
    normalized = normalized.replace(" Featuring ", ",");

    normalized
        .split(',')
        .map(|s| clean_text(s))
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
}

pub fn parse_age_flag(text: &str) -> Option<bool> {
    let lower = text.to_lowercase();
    if lower.contains("all ages") || lower.contains("all-ages") {
        Some(true)
    } else if lower.contains("21+")
        || lower.contains("18+")
        || lower.contains("21 and over")
        || lower.contains("21 & over")
    {
        Some(false)
    } else {
        None
    }
}

pub fn parse_datetime(date_text: &str, time_text: Option<&str>, tz: Tz) -> Option<DateTime<Tz>> {
    let cleaned_date = clean_text(date_text);
    if cleaned_date.is_empty() {
        return None;
    }
    let date = parse_naive_date(&cleaned_date)?;
    let time = parse_time_candidates(time_text, &[&cleaned_date])?;
    to_timezone_datetime(date, time, tz)
}

pub fn parse_named_time(text: &str, keyword: &str) -> Option<String> {
    let lowered = keyword.to_lowercase();
    for segment in text.split(['|', '/', ';']) {
        let segment_clean = clean_text(segment);
        if segment_clean.is_empty() {
            continue;
        }
        if segment_clean.to_lowercase().contains(&lowered) {
            if let Some(time) = find_first_time(&segment_clean) {
                return Some(time);
            }
        }
    }
    None
}

pub fn find_first_time(text: &str) -> Option<String> {
    let cleaned = clean_text(text);
    TIME_RE.captures(&cleaned).map(|caps| {
        let hour = caps.get(1).unwrap().as_str().parse::<u32>().unwrap_or(0);
        let minute = caps
            .get(2)
            .map(|m| m.as_str().parse::<u32>().unwrap_or(0))
            .unwrap_or(0);
        let period = caps.get(3).unwrap().as_str().to_uppercase();
        format!("{:02}:{:02} {}", hour, minute, period)
    })
}

pub fn combine_with_date(reference: &DateTime<Tz>, time_str: &str, tz: Tz) -> Option<String> {
    let time = parse_time_candidates(Some(time_str), &[])?;
    to_timezone_datetime(reference.date_naive(), time, tz).map(|dt| dt.to_rfc3339())
}

pub fn build_event(
    venue_id: &str,
    venue_name: &str,
    venue_url: &str,
    start_local: DateTime<Tz>,
    artists: Vec<String>,
    ticket_url: Option<String>,
    event_url: Option<String>,
    is_all_ages: Option<bool>,
    doors_local: Option<String>,
    extra: Value,
) -> Event {
    let start_utc = start_local.with_timezone(&Utc);
    let headliner = artists
        .first()
        .cloned()
        .unwrap_or_else(|| "unknown".to_string());
    let mut hasher = Sha256::new();
    hasher.update(venue_id.as_bytes());
    hasher.update(b"|");
    hasher.update(start_utc.to_rfc3339().as_bytes());
    hasher.update(b"|");
    hasher.update(headliner.as_bytes());
    let id = format!("{:x}", hasher.finalize());
    let event_url = event_url.or_else(|| ticket_url.clone());

    Event {
        id,
        source: venue_id.to_string(),
        venue_id: venue_id.to_string(),
        venue_name: Some(venue_name.to_string()),
        venue_url: Some(venue_url.to_string()),
        start_local: Some(start_local.to_rfc3339()),
        start_utc: start_utc.to_rfc3339(),
        doors_local,
        artists,
        is_all_ages,
        ticket_url,
        event_url,
        price_min_cents: None,
        price_max_cents: None,
        currency: None,
        tags: Vec::new(),
        scraped_at_utc: Utc::now().to_rfc3339(),
        extra,
    }
}

fn parse_time_candidates(primary: Option<&str>, others: &[&str]) -> Option<NaiveTime> {
    if let Some(value) = primary.and_then(|text| parse_naive_time_str(text)) {
        return Some(value);
    }
    for text in others {
        if let Some(value) = parse_naive_time_str(text) {
            return Some(value);
        }
    }
    None
}

fn parse_naive_time_str(text: &str) -> Option<NaiveTime> {
    let normalized = find_first_time(text)?;
    for fmt in ["%I:%M %p", "%I %p"].iter() {
        if let Ok(time) = NaiveTime::parse_from_str(&normalized, fmt) {
            return Some(time);
        }
    }
    None
}

fn parse_naive_date(input: &str) -> Option<NaiveDate> {
    let formats = [
        ("%m/%d/%Y", true),
        ("%m/%d/%y", true),
        ("%A %m/%d/%Y", true),
        ("%B %d, %Y", true),
        ("%b %d, %Y", true),
        ("%B %e, %Y", true),
        ("%b %e, %Y", true),
        ("%B %d", false),
        ("%b %d", false),
    ];

    for (fmt, has_year) in formats.iter() {
        if let Ok(mut date) = NaiveDate::parse_from_str(input, fmt) {
            if *has_year {
                return Some(date);
            }
            let current_year = Local::now().year();
            date = date.with_year(current_year)?;
            let today = Local::now().date_naive();
            if date < today {
                date = date.with_year(current_year + 1)?;
            }
            return Some(date);
        }
    }

    None
}

fn to_timezone_datetime(date: NaiveDate, time: NaiveTime, tz: Tz) -> Option<DateTime<Tz>> {
    let naive = NaiveDateTime::new(date, time);
    match tz.from_local_datetime(&naive) {
        LocalResult::Single(dt) => Some(dt),
        LocalResult::Ambiguous(dt, _) => Some(dt),
        LocalResult::None => None,
    }
}

pub fn empty_extra() -> Value {
    Value::Object(Map::new())
}

pub fn ensure_extra(mut extra: Map<String, Value>) -> Value {
    Value::Object(std::mem::take(&mut extra))
}

pub fn fail_if_empty<T>(venue_id: &str, events: Vec<T>) -> Result<Vec<T>> {
    if events.is_empty() {
        Err(anyhow!("no events scraped for {venue_id}"))
    } else {
        Ok(events)
    }
}
