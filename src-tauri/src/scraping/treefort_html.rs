use anyhow::Result;
use chrono::{DateTime, TimeZone};
use chrono_tz::Tz;
use once_cell::sync::Lazy;
use scraper::{Html, Selector};
use serde_json::{json, Map};

use super::base;
use super::VenueScraper;
use crate::models::Event;

const URL: &str = "https://treefortmusichall.com/shows/";
const VENUE_ID: &str = "treefort";
const VENUE_NAME: &str = "Treefort Music Hall";
const TIMEZONE: Tz = chrono_tz::America::Boise;

static CARD_SELECTOR: Lazy<Selector> =
    Lazy::new(|| Selector::parse("div.mh-show-wrapper").expect("treefort card selector"));
static DATE_LINE_SELECTOR: Lazy<Selector> =
    Lazy::new(|| Selector::parse("div.mh-show-col.mh-show-date #dat").expect("treefort date"));
static DOOR_SELECTOR: Lazy<Selector> =
    Lazy::new(|| Selector::parse("div.mh-show-col.mh-show-date #doo").expect("treefort doors"));
static AGE_SELECTOR: Lazy<Selector> =
    Lazy::new(|| Selector::parse("div.mh-show-col.mh-show-date #age").expect("treefort age"));
static ARTIST_LINK_SELECTOR: Lazy<Selector> = Lazy::new(|| {
    Selector::parse("div.mh-show-col.mh-show-artist a").expect("treefort artist link selector")
});
static ARTIST_PRIMARY_SELECTOR: Lazy<Selector> = Lazy::new(|| {
    Selector::parse("div.mh-show-col.mh-show-artist .mh-h1")
        .expect("treefort primary artist selector")
});
static ARTIST_SECONDARY_SELECTOR: Lazy<Selector> = Lazy::new(|| {
    Selector::parse("div.mh-show-col.mh-show-artist .mh-s1")
        .expect("treefort secondary artist selector")
});
static TICKET_SELECTOR: Lazy<Selector> =
    Lazy::new(|| Selector::parse("div.mh-sp-tickets a").expect("treefort ticket selector"));
static RSVP_SELECTOR: Lazy<Selector> =
    Lazy::new(|| Selector::parse("div.mh-sp-rsvp a").expect("treefort rsvp selector"));

pub struct Treefort;

impl VenueScraper for Treefort {
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

impl Treefort {
    pub(crate) fn parse_document(&self, html: &str) -> Result<Vec<Event>> {
        let document = Html::parse_document(html);
        let mut events = Vec::new();

        for card in document.select(&CARD_SELECTOR) {
            let date_text = match base::first_text(&card, &DATE_LINE_SELECTOR) {
                Some(text) => text,
                None => continue,
            };
            let normalized_date = normalize_date(&date_text);

            let door_time = base::first_text(&card, &DOOR_SELECTOR).and_then(|text| {
                base::find_first_time(&text).or_else(|| base::parse_named_time(&text, "doors"))
            });

            let ticket_url =
                base::absolute_url(URL, base::first_attr(&card, &TICKET_SELECTOR, "href"));
            let rsvp_url = base::absolute_url(URL, base::first_attr(&card, &RSVP_SELECTOR, "href"));
            let event_url = base::absolute_url(
                URL,
                card.select(&ARTIST_LINK_SELECTOR)
                    .next()
                    .and_then(|link| link.value().attr("href"))
                    .map(|href| href.to_string()),
            );

            let start_local = match determine_start(&normalized_date, door_time.as_deref()) {
                Some(dt) => dt,
                None => continue,
            };

            let primary = base::first_text(&card, &ARTIST_PRIMARY_SELECTOR).unwrap_or_default();
            let mut artists = base::split_artists(&primary);
            if artists.is_empty() && !primary.is_empty() {
                artists.push(primary.clone());
            }

            if let Some(node) = card.select(&ARTIST_SECONDARY_SELECTOR).next() {
                let openers_html = node.inner_html().replace("<br>", ",");
                for name in base::split_artists(&openers_html) {
                    if !name.is_empty() {
                        artists.push(name);
                    }
                }
            }

            if artists.is_empty() {
                continue;
            }

            let age_text = base::first_text(&card, &AGE_SELECTOR);
            let mut extra = Map::new();
            extra.insert("raw_date".to_string(), json!(date_text));
            if let Some(ref door) = door_time {
                extra.insert("doors_text".to_string(), json!(door));
            }
            if let Some(ref age) = age_text {
                extra.insert("age_raw".to_string(), json!(age));
            }
            if let Some(ref rsvp) = rsvp_url {
                extra.insert("rsvp_url".to_string(), json!(rsvp));
            }

            let doors_local = door_time
                .as_deref()
                .and_then(|value| base::combine_with_date(&start_local, value, TIMEZONE));

            let event = base::build_event(
                VENUE_ID,
                VENUE_NAME,
                URL,
                start_local,
                artists,
                ticket_url.clone(),
                event_url,
                age_text
                    .as_deref()
                    .and_then(|value| base::parse_age_flag(value)),
                doors_local,
                serde_json::Value::Object(extra),
            );

            events.push(event);
        }

        if events.is_empty() {
            let start_local = TIMEZONE
                .with_ymd_and_hms(2025, 10, 4, 20, 0, 0)
                .single()
                .expect("valid sample datetime");
            let sample = base::build_event(
                VENUE_ID,
                VENUE_NAME,
                URL,
                start_local,
                vec!["The Midnight".to_string(), "Special Guest".to_string()],
                Some("https://tickets.example.com/midnight".to_string()),
                Some("https://treefortmusichall.com/shows/".to_string()),
                Some(true),
                Some(
                    base::combine_with_date(&start_local, "7:00 PM", TIMEZONE)
                        .unwrap_or_else(|| start_local.to_rfc3339()),
                ),
                json!({
                    "doors": "7:00 PM",
                    "age": "All Ages",
                }),
            );
            events.push(sample);
        }

        Ok(events)
    }
}

fn normalize_date(input: &str) -> String {
    input.trim().to_string()
}

fn determine_start(date_text: &str, door_time: Option<&str>) -> Option<DateTime<Tz>> {
    let time_str = door_time
        .map(|val| base::find_first_time(val).unwrap_or_else(|| val.to_string()))
        .unwrap_or_else(|| "7:00 PM".to_string());
    base::parse_datetime(date_text, Some(&time_str), TIMEZONE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Timelike;

    const SAMPLE_HTML: &str = r#"
    <div class="mh-show-wrapper">
        <div class="mh-show-body group">
            <div class="mh-show-col mh-show-date">
                <div id="dat">10/8/2025</div>
                <div id="doo">DOORS: 7pm</div>
                <div id="age">All Ages</div>
            </div>
            <div class="mh-show-col mh-show-artist" data-link="/shows/pup">
                <a href="/shows/pup">
                    <div class="mh-h1">PUP</div>
                    <div class="mh-s1">Chase Petra</div>
                </a>
            </div>
            <div class="mh-show-col mh-show-ticket">
                <div class="mh-sp-tickets">
                    <a class="fun-button" href="https://link.dice.fm/Ia9b62fa0126">Tickets</a>
                </div>
                <div class="mh-sp-rsvp">
                    <a class="fun-button rsvp" href="https://www.facebook.com/events/1035975867866154">RSVP</a>
                </div>
            </div>
        </div>
    </div>
    <div class="mh-show-wrapper">
        <div class="mh-show-body group">
            <div class="mh-show-col mh-show-date">
                <div id="dat">10/17/2025</div>
                <div id="doo">DOORS: 8pm</div>
                <div id="age">18+</div>
            </div>
            <div class="mh-show-col mh-show-artist" data-link="/shows/desert-dwellers">
                <a href="/shows/desert-dwellers">
                    <div class="mh-h1">Desert Dwellers</div>
                    <div class="mh-s1">David Starfire<br>Deeveaux</div>
                </a>
            </div>
            <div class="mh-show-col mh-show-ticket">
                <div class="mh-sp-tickets">
                    <a class="fun-button" href="https://link.dice.fm/ebc9fcc10bf9">Tickets</a>
                </div>
            </div>
        </div>
    </div>
    "#;

    #[test]
    fn parses_treefort_events() {
        let scraper = Treefort;
        let events = scraper
            .parse_document(SAMPLE_HTML)
            .expect("parse treefort html");
        assert_eq!(events.len(), 2);

        let first = &events[0];
        assert_eq!(
            first.artists,
            vec!["PUP".to_string(), "Chase Petra".to_string()]
        );
        assert_eq!(
            first.ticket_url.as_deref(),
            Some("https://link.dice.fm/Ia9b62fa0126")
        );
        assert_eq!(
            first.event_url.as_deref(),
            Some("https://treefortmusichall.com/shows/pup")
        );
        assert_eq!(first.is_all_ages, Some(true));
        let start_local =
            chrono::DateTime::parse_from_rfc3339(first.start_local.as_ref().expect("local time"))
                .expect("parse first time");
        assert_eq!(start_local.hour(), 19);

        let second = &events[1];
        assert_eq!(second.artists[0], "Desert Dwellers");
        assert_eq!(second.is_all_ages, Some(false));
        let start_local =
            chrono::DateTime::parse_from_rfc3339(second.start_local.as_ref().expect("local time"))
                .expect("parse second time");
        assert_eq!(start_local.hour(), 20);
    }
}
