use anyhow::Result;

use super::VenueScraper;
use crate::models::Event;

pub struct PineBox;

impl VenueScraper for PineBox {
    fn venue_id(&self) -> &'static str {
        "pine_box"
    }

    fn venue_name(&self) -> &'static str {
        "Pine Box Rock Shop"
    }

    fn venue_url(&self) -> &'static str {
        "https://pineboxrockshop.com/"
    }

    fn fetch(&self) -> Result<Vec<Event>> {
        Ok(Vec::new())
    }
}
