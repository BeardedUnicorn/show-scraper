# Venue Scraping Information

This document tracks implementation details, selectors, and normalization notes for all supported venues in the Tauri v2 ShowScraper app.

---

## Treefort Music Hall
**URL:** https://treefortmusichall.com/shows/

### Selectors
- **Card Selector:** `div.show-card, .show-card`
- **Artist Selector:** `.artist, .show-card .artist`
- **Date/Time Selector:** `.date-time, .show-card .date-time`
- **Doors Selector:** `.doors, .door-time, .show-card .doors`
- **Age Restriction:** `.ages, .age, .show-card .ages`
- **Ticket Link:** `a.ticket, .show-card a.ticket, a.Tickets`
- **RSVP Link:** `a.rsvp, .show-card a.rsvp, a.RSVP`

### Notes
- Extract artist names, date, time, and ticket link.
- Timezone: `America/Boise`.
- Age restriction values (e.g., "18+", "All Ages") mapped to `is_all_ages` boolean.
- Normalize start time by removing the “DOORS” phrase and converting to ISO 8601.

### Example Output
```json
{
  "source": "treefort",
  "venue_name": "Treefort Music Hall",
  "start": "Saturday 10/4/2025 8:00 PM",
  "timezone": "America/Boise",
  "artists": ["The Midnight", "Special Guest"],
  "ticket_url": "https://tickets.example.com/midnight",
  "extra": {"doors": "7:00 PM", "age": "All Ages"}
}
```

---

## Revolution Concert House
**URL:** https://cttouringid.com/tm-venue/revolution-concert-house-and-event-center/

### Selectors
- **Event Row:** `div.show, .event, li`
- **Date:** `div.date, .date`
- **Time:** `div.time, .time, .doors-show`
- **Artist:** `div.artist, .artist`
- **Ticket:** `a[href*='ticket'], a.buy-tickets, a.Tickets`

### Notes
- Combine date + time fields for normalization.
- Extract doors and show times separately if both present.
- Timezone: `America/Boise`.

### Example Output
```json
{
  "source": "revolution",
  "venue_name": "Revolution Concert House",
  "start": "2025-10-15T20:00:00-07:00",
  "timezone": "America/Boise",
  "artists": ["Dance Gavin Dance"],
  "ticket_url": "https://ticketmaster.com/event/12345",
  "extra": {"doors": "7:00 PM", "show": "8:00 PM"}
}
```

---

## Knitting Factory Boise
**URL:** https://bo.knittingfactory.com/

### Selectors
- **Event Block:** `.upcoming-event, .event`
- **Date:** `.date, .event-date`
- **Show Time:** `.show, .show-time`
- **Artist:** `.title, .artist`
- **Ticket Link:** `a[href*='ticket'], a.buy-tickets`

### Notes
- HTML is partially static; consider JSON API if discovered.
- Parse date and show time text into a proper datetime string.
- Timezone: `America/Boise`.

### Example Output
```json
{
  "source": "knitboise",
  "venue_name": "Knitting Factory Boise",
  "start": "2025-11-02T19:00:00-07:00",
  "timezone": "America/Boise",
  "artists": ["Of Monsters and Men"],
  "ticket_url": "https://ticketweb.com/event/12345",
  "extra": {"show_time": "7:00 PM"}
}
```

---

## Normalization Notes
- Convert all datetimes to both local and UTC.
- Compute event ID as SHA256 hash of `venue_id|start_utc|main_artist`.
- `price_text` should be parsed for numeric ranges if discovered.
- Store `age_restriction` in `extra` and map it to `is_all_ages` in normalized event.
- Enforce consistent field names across venues.

---

## Testing & QA
- Run scrapers individually: `showscraper scrape --venue treefort`.
- Verify that each scraper populates required fields (`artists`, `start`, `ticket_url`).
- Store sample outputs in `tests/fixtures/` for snapshot comparisons.
- Confirm `normalize_event()` handles all date formats.

---

## Future Venues
- **The Olympic Venue (Boise)** – Potential addition.
- **Neurolux** – Check for JSON or iCal feeds.
- **Ford Idaho Center** – Use Ticketmaster API if supported.

---

