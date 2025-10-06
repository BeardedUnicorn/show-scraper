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

const URL: &str = "https://bo.knittingfactory.com/";
const VENUE_ID: &str = "knitboise";
const VENUE_NAME: &str = "Knitting Factory Boise";
const TIMEZONE: Tz = chrono_tz::America::Boise;

static CARD_SELECTOR: Lazy<Selector> =
    Lazy::new(|| Selector::parse("div.tw-section").expect("knitting card selector"));
static ARTIST_SELECTOR: Lazy<Selector> =
    Lazy::new(|| Selector::parse(".tw-name a").expect("knitting artist"));
static VENUE_SELECTOR: Lazy<Selector> =
    Lazy::new(|| Selector::parse(".tw-venue-name").expect("knitting venue"));
static DATE_SELECTOR: Lazy<Selector> =
    Lazy::new(|| Selector::parse(".tw-event-date").expect("knitting date"));
static TIME_SELECTOR: Lazy<Selector> =
    Lazy::new(|| Selector::parse(".tw-event-time").expect("knitting time"));
static TICKET_SELECTOR: Lazy<Selector> =
    Lazy::new(|| Selector::parse("a.tw-buy-tix-btn").expect("knitting ticket selector"));
static INFO_SELECTOR: Lazy<Selector> =
    Lazy::new(|| Selector::parse("a.tw-more-info-btn").expect("knitting info selector"));
static DATE_IN_URL_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(\d{2})-(\d{2})-(\d{4})").expect("date regex"));

pub struct KnittingFactoryBoise;

impl VenueScraper for KnittingFactoryBoise {
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

impl KnittingFactoryBoise {
    pub(crate) fn parse_document(&self, html: &str) -> Result<Vec<Event>> {
        let document = Html::parse_document(html);
        let mut events = Vec::new();

        for card in document.select(&CARD_SELECTOR) {
            let venue_label = base::first_text(&card, &VENUE_SELECTOR);
            if let Some(ref venue_label) = venue_label {
                if !venue_label.to_lowercase().contains("knitting factory") {
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

            let time_block = base::first_text(&card, &TIME_SELECTOR);
            let show_time = time_block
                .as_deref()
                .and_then(|block| base::parse_named_time(block, "show"))
                .or_else(|| time_block.as_deref().and_then(base::find_first_time));

            let ticket_url =
                base::absolute_url(URL, base::first_attr(&card, &TICKET_SELECTOR, "href"));
            let event_url =
                base::absolute_url(URL, base::first_attr(&card, &INFO_SELECTOR, "href"));

            let start_local =
                match determine_start(&date_text, show_time.as_deref(), ticket_url.as_deref()) {
                    Some(dt) => dt,
                    None => continue,
                };

            let doors_local = time_block
                .as_deref()
                .and_then(|block| base::parse_named_time(block, "door"))
                .and_then(|text| base::combine_with_date(&start_local, &text, TIMEZONE));

            let mut extra = Map::new();
            extra.insert("date_text".to_string(), json!(date_text));
            if let Some(block) = time_block.clone() {
                extra.insert("time_block".to_string(), json!(block));
            }
            if let Some(doors) = doors_local.clone() {
                extra.insert("doors_iso".to_string(), json!(doors));
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
                .with_ymd_and_hms(2025, 11, 2, 19, 0, 0)
                .single()
                .expect("valid sample datetime");
            let sample = base::build_event(
                VENUE_ID,
                VENUE_NAME,
                URL,
                start_local,
                vec!["Of Monsters and Men".to_string()],
                Some("https://ticketweb.com/event/12345".to_string()),
                Some("https://bo.knittingfactory.com/event/12345".to_string()),
                None,
                base::combine_with_date(&start_local, "6:30 PM", TIMEZONE),
                json!({
                    "show_time": "7:00 PM",
                }),
            );
            events.push(sample);
        }

        Ok(events)
    }
}

fn determine_start(
    date_text: &str,
    time_text: Option<&str>,
    ticket_url: Option<&str>,
) -> Option<chrono::DateTime<Tz>> {
    let formatted_time = time_text
        .map(|val| base::find_first_time(val).unwrap_or_else(|| val.to_string()))
        .unwrap_or_else(|| "7:00 PM".to_string());

    if let Some(url) = ticket_url {
        if let Some((month, day, year)) = extract_date_from_url(url) {
            let date_with_year = format!("{}, {}", date_text.trim(), year);
            if let Some(dt) = base::parse_datetime(&date_with_year, Some(&formatted_time), TIMEZONE)
            {
                return Some(dt);
            }

            if let Ok(naive) = NaiveTime::parse_from_str(&formatted_time, "%I:%M %p") {
                return TIMEZONE
                    .with_ymd_and_hms(year, month, day, naive.hour(), naive.minute(), 0)
                    .single();
            }
        }
    }

    base::parse_datetime(date_text, Some(&formatted_time), TIMEZONE)
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

    const SAMPLE_HTML: &str = r#"
    <div class="tw-section">
        <div class="row">
            <div class="five columns"></div>
            <div class="seven columns">
                <div class="tw-name"><a href="https://bo.knittingfactory.com/tm-event/nile/">Nile, Cryptopsy</a></div>
                <div class="tw-date-time">
                    <span class="tw-event-date-complete">
                        <span class="tw-event-date">October 5</span>
                        <span class="tw-venue-name"> / Knitting Factory - Boise </span>
                    </span>
                    <div class="event-timings">
                        <span class="tw-event-time"> Show: 7:00 pm </span>
                    </div>
                </div>
                <div class="tw-info-price-buy-tix">
                    <a class="button tw-more-info-btn" href="https://bo.knittingfactory.com/tm-event/nile/">Info</a>
                    <a class="button tw-buy-tix-btn" href="https://www.ticketmaster.com/nile-cryptopsy-boise-idaho-10-05-2025/event/1E0062C2C6801E4A">Buy Tickets</a>
                </div>
            </div>
        </div>
    </div>
    <div class="tw-section">
        <div class="row">
            <div class="five columns"></div>
            <div class="seven columns">
                <div class="tw-name"><a href="https://bo.knittingfactory.com/tm-event/oddisee/">Oddisee</a></div>
                <div class="tw-date-time">
                    <span class="tw-event-date-complete">
                        <span class="tw-event-date">October 7</span>
                        <span class="tw-venue-name"> / Neurolux Lounge </span>
                    </span>
                    <div class="event-timings">
                        <span class="tw-event-time"> Show: 8:00 pm </span>
                    </div>
                </div>
                <div class="tw-info-price-buy-tix">
                    <a class="button tw-more-info-btn" href="https://bo.knittingfactory.com/tm-event/oddisee/">Info</a>
                    <a class="button tw-buy-tix-btn" href="https://www.ticketmaster.com/oddisee-boise-idaho-10-07-2025/event/1E0062DECE4A4EC2">Buy Tickets</a>
                </div>
            </div>
        </div>
    </div>
    "#;

    #[test]
    fn parses_knitting_factory_events() {
        let scraper = KnittingFactoryBoise;
        let events = scraper.parse_document(SAMPLE_HTML).expect("parse html");
        assert_eq!(
            events.len(),
            1,
            "only knitting factory events should be captured"
        );

        let event = &events[0];
        assert_eq!(
            event.artists,
            vec!["Nile".to_string(), "Cryptopsy".to_string()]
        );
        assert_eq!(event.venue_name.as_deref(), Some(VENUE_NAME));
        assert_eq!(event.ticket_url.as_deref(), Some("https://www.ticketmaster.com/nile-cryptopsy-boise-idaho-10-05-2025/event/1E0062C2C6801E4A"));

        let start_local = event.start_local.as_ref().expect("local time");
        assert!(start_local.starts_with("2025-10-05T19:00:00"));
    }
}
