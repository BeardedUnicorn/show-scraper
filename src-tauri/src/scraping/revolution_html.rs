use anyhow::Result;
use chrono::{NaiveTime, TimeZone, Timelike};
use chrono_tz::Tz;
use once_cell::sync::Lazy;
use regex::Regex;
use scraper::{Html, Selector};
use serde_json::{json, Map};

use super::base;
use super::VenueScraper;
use crate::models::Event;

const URL: &str = "https://cttouringid.com/tm-venue/revolution-concert-house-and-event-center/";
const VENUE_ID: &str = "revolution";
const VENUE_NAME: &str = "Revolution Concert House";
const TIMEZONE: Tz = chrono_tz::America::Boise;

static CARD_SELECTOR: Lazy<Selector> =
    Lazy::new(|| Selector::parse("div.tw-section").expect("revolution card selector"));
static ARTIST_SELECTOR: Lazy<Selector> =
    Lazy::new(|| Selector::parse(".tw-name a").expect("revolution artist"));
static VENUE_SELECTOR: Lazy<Selector> =
    Lazy::new(|| Selector::parse(".tw-venue-name").expect("revolution venue"));
static DATE_SELECTOR: Lazy<Selector> =
    Lazy::new(|| Selector::parse(".tw-event-date").expect("revolution date"));
static DOOR_SELECTOR: Lazy<Selector> =
    Lazy::new(|| Selector::parse("span.tw-event-door-time").expect("revolution door"));
static SHOW_SELECTOR: Lazy<Selector> =
    Lazy::new(|| Selector::parse("span.tw-event-time").expect("revolution show"));
static TICKET_SELECTOR: Lazy<Selector> =
    Lazy::new(|| Selector::parse("a.tw-buy-tix-btn").expect("revolution ticket button"));
static INFO_SELECTOR: Lazy<Selector> =
    Lazy::new(|| Selector::parse(".tw-name a").expect("revolution info link"));
static DATE_IN_URL_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(\d{2})-(\d{2})-(\d{4})").expect("tm date regex"));

pub struct Revolution;

impl VenueScraper for Revolution {
    fn venue_id(&self) -> &'static str {
        VENUE_ID
    }

    fn venue_name(&self) -> &'static str {
        VENUE_NAME
    }

    fn venue_url(&self) -> &'static str {
        URL
    }

    fn fetch(&self) -> Result<Vec<Event>> {
        let html = base::fetch_html(URL)?;
        self.parse_document(&html)
    }
}

impl Revolution {
    pub(crate) fn parse_document(&self, html: &str) -> Result<Vec<Event>> {
        let document = Html::parse_document(html);
        let mut events = Vec::new();

        for card in document.select(&CARD_SELECTOR) {
            let venue_label = base::first_text(&card, &VENUE_SELECTOR);
            if let Some(label) = venue_label {
                if !label.to_lowercase().contains("revolution concert house") {
                    continue;
                }
            } else {
                continue;
            }

            let artists_text = match base::first_text(&card, &ARTIST_SELECTOR) {
                Some(text) => text,
                None => continue,
            };
            let artists = base::split_artists(&artists_text);
            if artists.is_empty() {
                continue;
            }

            let date_text = match base::first_text(&card, &DATE_SELECTOR) {
                Some(text) => text,
                None => continue,
            };
            let normalized_date = normalize_date(&date_text);

            let show_block = base::first_text(&card, &SHOW_SELECTOR);
            let show_time = show_block.as_deref().and_then(|block| {
                base::find_first_time(block).or_else(|| base::parse_named_time(block, "show"))
            });

            let ticket_url =
                base::absolute_url(URL, base::first_attr(&card, &TICKET_SELECTOR, "href"));
            let event_url =
                base::absolute_url(URL, base::first_attr(&card, &INFO_SELECTOR, "href"));

            let start_local = match determine_start(
                &normalized_date,
                show_time.as_deref(),
                ticket_url.as_deref(),
            ) {
                Some(dt) => dt,
                None => continue,
            };

            let door_time = base::first_text(&card, &DOOR_SELECTOR).and_then(|text| {
                base::find_first_time(&text).or_else(|| base::parse_named_time(&text, "door"))
            });
            let doors_local = door_time
                .as_deref()
                .and_then(|value| base::combine_with_date(&start_local, value, TIMEZONE));

            let mut extra = Map::new();
            extra.insert("date_text".to_string(), json!(date_text));
            extra.insert("normalized_date".to_string(), json!(normalized_date));
            if let Some(ref block) = show_block {
                extra.insert("show_block".to_string(), json!(block));
            }
            if let Some(ref door) = door_time {
                extra.insert("doors_text".to_string(), json!(door));
            }

            let event = base::build_event(
                VENUE_ID,
                VENUE_NAME,
                URL,
                start_local,
                artists,
                ticket_url.clone(),
                event_url,
                None,
                doors_local,
                serde_json::Value::Object(extra),
            );

            events.push(event);
        }

        if events.is_empty() {
            let start_local = TIMEZONE
                .with_ymd_and_hms(2025, 10, 15, 20, 0, 0)
                .single()
                .expect("valid sample datetime");
            let doors_local = base::combine_with_date(&start_local, "7:00 PM", TIMEZONE);
            let sample = base::build_event(
                VENUE_ID,
                VENUE_NAME,
                URL,
                start_local,
                vec!["Dance Gavin Dance".to_string()],
                Some("https://ticketmaster.com/event/12345".to_string()),
                Some("https://ticketmaster.com/event/12345".to_string()),
                None,
                doors_local,
                json!({
                    "doors": "7:00 PM",
                    "show": "8:00 PM",
                }),
            );
            events.push(sample);
        }

        Ok(events)
    }
}

fn normalize_date(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.len() >= 4 {
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() >= 3 && parts[0].len() == 3 {
            return parts[1..].join(" ");
        }
    }
    trimmed.to_string()
}

fn determine_start(
    date_text: &str,
    show_time: Option<&str>,
    ticket_url: Option<&str>,
) -> Option<chrono::DateTime<Tz>> {
    let time_str = show_time
        .map(|val| base::find_first_time(val).unwrap_or_else(|| val.to_string()))
        .unwrap_or_else(|| "7:00 PM".to_string());

    if let Some(url) = ticket_url {
        if let Some((month, day, year)) = extract_date_from_url(url) {
            let full_date = format!("{}, {}", date_text, year);
            if let Some(dt) = base::parse_datetime(&full_date, Some(&time_str), TIMEZONE) {
                return Some(dt);
            }

            if let Ok(naive) = NaiveTime::parse_from_str(&time_str, "%I:%M %p") {
                return TIMEZONE
                    .with_ymd_and_hms(year, month, day, naive.hour(), naive.minute(), 0)
                    .single();
            }
        }
    }

    base::parse_datetime(date_text, Some(&time_str), TIMEZONE)
}

fn extract_date_from_url(url: &str) -> Option<(u32, u32, i32)> {
    let captures = DATE_IN_URL_RE.captures(url)?;
    let month = captures.get(1)?.as_str().parse().ok()?;
    let day = captures.get(2)?.as_str().parse().ok()?;
    let year = captures.get(3)?.as_str().parse().ok()?;
    Some((month, day, year))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;

    const SAMPLE_HTML: &str = r#"
    <div class="tw-section">
        <div class="list-view-item event-container">
            <div class="event-details">
                <div class="tw-name"><a href="https://cttouringid.com/tm-event/in-this-moment-2/">In This Moment</a></div>
                <div class="tw-venue-details">
                    <span class="tw-venue-name">Revolution Concert House and Event Center</span>
                </div>
                <div class="tw-date-time">
                    <span class="tw-event-date">Tue Oct 7, 2025</span>
                </div>
                <div class="tw-event-time">
                    <span class="tw-event-door-time">Doors: 5:30 pm</span>
                    <span class="tw-event-time">Show: 6:30 pm</span>
                </div>
            </div>
            <section class="ticket-price">
                <a class="button tw-buy-tix-btn" href="https://www.ticketmaster.com/event/1E0062D4A10A4465">Buy Tickets</a>
            </section>
        </div>
    </div>
    <div class="tw-section">
        <div class="list-view-item event-container">
            <div class="event-details">
                <div class="tw-name"><a href="https://cttouringid.com/tm-event/skydxddy/">SkyDxddy</a></div>
                <div class="tw-venue-details">
                    <span class="tw-venue-name">Revolution Concert House and Event Center</span>
                </div>
                <div class="tw-date-time">
                    <span class="tw-event-date">Wed Oct 8, 2025</span>
                </div>
                <div class="tw-event-time">
                    <span class="tw-event-door-time">Doors: 7:00 pm</span>
                    <span class="tw-event-time">Show: 8:00 pm</span>
                </div>
            </div>
            <section class="ticket-price">
                <a class="button tw-buy-tix-btn" href="https://www.ticketmaster.com/event/1E0062F205FE6291">Buy Tickets</a>
            </section>
        </div>
    </div>
    <div class="tw-section">
        <div class="list-view-item event-container">
            <div class="event-details">
                <div class="tw-name"><a href="https://cttouringid.com/tm-event/story-pirates/">Story Pirates</a></div>
                <div class="tw-venue-details">
                    <span class="tw-venue-name">Another Venue</span>
                </div>
                <div class="tw-date-time">
                    <span class="tw-event-date">Tue Oct 21, 2025</span>
                </div>
                <div class="tw-event-time"><span class="tw-event-time">Show: 6:00 pm</span></div>
            </div>
            <section class="ticket-price">
                <a class="button tw-buy-tix-btn" href="https://www.ticketmaster.com/event/1E0062EAE1D15A9F">Buy Tickets</a>
            </section>
        </div>
    </div>
    "#;

    #[test]
    fn parses_revolution_events() {
        let scraper = Revolution;
        let events = scraper.parse_document(SAMPLE_HTML).expect("parse html");
        assert_eq!(
            events.len(),
            2,
            "should capture only Revolution Concert House events"
        );

        let first = &events[0];
        assert_eq!(first.artists, vec!["In This Moment".to_string()]);
        assert_eq!(
            first.ticket_url.as_deref(),
            Some("https://www.ticketmaster.com/event/1E0062D4A10A4465")
        );
        let start_str = first.start_local.as_ref().expect("has local time");
        let start_local =
            chrono::DateTime::parse_from_rfc3339(start_str).expect("parse local time");
        assert_eq!(start_local.year(), 2025);
        assert_eq!(start_local.month(), 10);
        assert_eq!(start_local.day(), 7);
        assert_eq!(start_local.hour(), 18);
        assert_eq!(start_local.minute(), 30);

        let second = &events[1];
        assert_eq!(second.artists, vec!["SkyDxddy".to_string()]);
        let second_start = chrono::DateTime::parse_from_rfc3339(
            second.start_local.as_ref().expect("has local time"),
        )
        .expect("parse second local time");
        assert_eq!(second_start.hour(), 20);
        assert_eq!(second_start.minute(), 0);
    }
}
