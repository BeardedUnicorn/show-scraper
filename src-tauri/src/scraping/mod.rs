pub mod base;
pub mod fox_theater_ics;
pub mod knitting_factory_html;
pub mod pine_box_html;
pub mod revolution_html;
pub mod treefort_html;

use anyhow::Error;

use crate::models::Event;

pub trait VenueScraper: Send + Sync {
    fn venue_id(&self) -> &'static str;
    fn venue_name(&self) -> &'static str;
    fn venue_url(&self) -> &'static str;
    fn fetch(&self) -> anyhow::Result<Vec<Event>>;
}

#[derive(Clone, serde::Serialize)]
pub struct ScraperInfo {
    pub id: String,
    pub name: String,
    pub url: String,
}

fn active_scrapers() -> Vec<Box<dyn VenueScraper>> {
    vec![
        Box::new(treefort_html::Treefort),
        Box::new(revolution_html::Revolution),
        Box::new(knitting_factory_html::KnittingFactoryBoise),
    ]
}

pub fn list_scrapers() -> Vec<ScraperInfo> {
    active_scrapers()
        .into_iter()
        .map(|scraper| ScraperInfo {
            id: scraper.venue_id().to_string(),
            name: scraper.venue_name().to_string(),
            url: scraper.venue_url().to_string(),
        })
        .collect()
}

fn find_scraper(id: &str) -> Option<Box<dyn VenueScraper>> {
    for scraper in active_scrapers() {
        if scraper.venue_id() == id {
            return Some(scraper);
        }
    }
    None
}

pub fn run_all() -> anyhow::Result<Vec<Event>> {
    let mut events = Vec::new();
    let mut errors: Vec<(String, Error)> = Vec::new();

    for scraper in active_scrapers() {
        let venue_id = scraper.venue_id().to_string();
        match scraper.fetch() {
            Ok(mut scraped) => events.append(&mut scraped),
            Err(err) => {
                errors.push((venue_id, err));
            }
        }
    }

    if events.is_empty() && !errors.is_empty() {
        let joined = errors
            .into_iter()
            .map(|(id, err)| format!("{id}: {err}"))
            .collect::<Vec<_>>()
            .join("; ");
        return Err(anyhow::anyhow!("scrapers failed: {joined}"));
    }

    Ok(events)
}

pub fn run_single(id: &str) -> anyhow::Result<Vec<Event>> {
    let scraper = find_scraper(id).ok_or_else(|| anyhow::anyhow!("unknown venue id: {id}"))?;
    scraper.fetch()
}
