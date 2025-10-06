use anyhow::Result;

use super::VenueScraper;
use crate::models::Event;

pub struct FoxTheater;

impl VenueScraper for FoxTheater {
    fn venue_id(&self) -> &'static str {
        "fox_theater"
    }

    fn venue_name(&self) -> &'static str {
        "Fox Theater"
    }

    fn venue_url(&self) -> &'static str {
        "https://www.foxtheatre.org/"
    }

    fn fetch(&self) -> Result<Vec<Event>> {
        Ok(Vec::new())
    }
}
