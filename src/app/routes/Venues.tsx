import type { Venue } from "../../lib/types";
import { parseVenues } from "../../lib/types";
import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

type VenueState = {
  running: boolean;
  lastRun?: string;
  lastCount?: number;
  error?: string;
};

type Toast = { kind: "success" | "error"; message: string } | null;

export default function Venues() {
  const [venues, setVenues] = useState<Venue[]>([]);
  const [loading, setLoading] = useState(false);
  const [bulkBusy, setBulkBusy] = useState(false);
  const [states, setStates] = useState<Record<string, VenueState>>({});
  const [toast, setToast] = useState<Toast>(null);

  const initVenueState = useCallback((items: Venue[]) => {
    setStates((prev) => {
      const next: Record<string, VenueState> = { ...prev };
      for (const venue of items) {
        if (!next[venue.id]) {
          next[venue.id] = { running: false };
        }
      }
      return next;
    });
  }, []);

  const loadVenues = useCallback(async () => {
    setLoading(true);
    try {
      const response = await invoke("list_venues");
      const parsed = parseVenues(response);
      setVenues(parsed);
      initVenueState(parsed);
    } catch (error) {
      console.error(error);
      setToast({ kind: "error", message: "Failed to load venues." });
    } finally {
      setLoading(false);
    }
  }, [initVenueState]);

  useEffect(() => {
    loadVenues();
  }, [loadVenues]);

  const handleScrape = useCallback(
    async (venueId: string) => {
      setStates((prev) => {
        const current = prev[venueId] ?? { running: false };
        return {
          ...prev,
          [venueId]: {
            ...current,
            running: true,
            error: undefined,
          },
        };
      });
      setToast(null);
      try {
        const count = await invoke<number>("scrape_venue", {
          venueId,
          venue_id: venueId,
        });
        const timestamp = new Date().toLocaleString();
        setStates((prev) => {
          const current = prev[venueId] ?? { running: false };
          return {
            ...prev,
            [venueId]: {
              ...current,
              running: false,
              lastRun: timestamp,
              lastCount: count,
              error: undefined,
            },
          };
        });
        setToast({
          kind: "success",
          message: `Scraped ${count} event${count === 1 ? "" : "s"}.`,
        });
      } catch (error) {
        console.error(error);
        setStates((prev) => {
          const current = prev[venueId] ?? { running: false };
          return {
            ...prev,
            [venueId]: {
              ...current,
              running: false,
              error: "Scrape failed",
            },
          };
        });
        setToast({ kind: "error", message: "Scrape failed. Check logs." });
      }
    },
    []
  );

  const handleScrapeAll = useCallback(async () => {
    setBulkBusy(true);
    setToast(null);
    setStates((prev) => {
      const next: Record<string, VenueState> = {};
      for (const [id, state] of Object.entries(prev)) {
        next[id] = { ...state, running: true, error: undefined };
      }
      return next;
    });
    try {
      const total = await invoke<number>("scrape_all");
      const timestamp = new Date().toLocaleString();
      setStates((prev) => {
        const next: Record<string, VenueState> = {};
        for (const [id, state] of Object.entries(prev)) {
          next[id] = {
            ...state,
            running: false,
            lastRun: timestamp,
            error: undefined,
          };
        }
        return next;
      });
      setToast({
        kind: "success",
        message: `Scraped ${total} events across all venues.`,
      });
    } catch (error) {
      console.error(error);
      setStates((prev) => {
        const next: Record<string, VenueState> = {};
        for (const [id, state] of Object.entries(prev)) {
          next[id] = { ...state, running: false };
        }
        return next;
      });
      setToast({ kind: "error", message: "Global scrape failed." });
    } finally {
      setBulkBusy(false);
    }
  }, []);

  const sortedVenues = useMemo(() => {
    return [...venues].sort((a, b) => a.name.localeCompare(b.name));
  }, [venues]);

  return (
    <div className="space-y-6">
      <div className="section-header">
        <div className="section-header__title">Venues</div>
        <div className="actions" style={{ display: "flex", gap: "12px" }}>
          <button className="button" onClick={loadVenues} disabled={loading || bulkBusy}>
            {loading ? "Refreshing…" : "Refresh"}
          </button>
          <button className="button" onClick={handleScrapeAll} disabled={bulkBusy || loading}>
            {bulkBusy ? "Scraping…" : "Scrape All"}
          </button>
        </div>
      </div>

      {toast && (
        <div className={`toast${toast.kind === "error" ? " toast--error" : ""}`}>
          {toast.message}
        </div>
      )}

      {loading && !venues.length ? (
        <div className="card">Loading venues…</div>
      ) : !sortedVenues.length ? (
        <div className="card">No venues configured yet.</div>
      ) : (
        <div className="pending-grid">
          {sortedVenues.map((venue) => {
            const state = states[venue.id] || { running: false };
            return (
              <VenueCard
                key={venue.id}
                venue={venue}
                state={state}
                disabled={bulkBusy || loading}
                onScrape={handleScrape}
              />
            );
          })}
        </div>
      )}
    </div>
  );
}

type VenueCardProps = {
  venue: Venue;
  state: VenueState;
  disabled: boolean;
  onScrape: (venueId: string) => Promise<void>;
};

function VenueCard({ venue, state, disabled, onScrape }: VenueCardProps) {
  const busy = state.running || disabled;
  return (
    <section className="card" style={{ display: "flex", flexDirection: "column", gap: "12px" }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start" }}>
        <div>
          <div className="pending-card__title" style={{ fontSize: "18px" }}>
            {venue.name}
          </div>
          <a href={venue.url} className="pending-card__meta" target="_blank" rel="noreferrer">
            {venue.url}
          </a>
        </div>
        <button
          className="button"
          disabled={busy}
          onClick={() => onScrape(venue.id)}
          style={{ minWidth: "120px", justifyContent: "center" }}
        >
          {state.running ? "Scraping…" : "Run Scrape"}
        </button>
      </div>
      <div className="pending-card__meta" style={{ display: "flex", gap: "16px", flexWrap: "wrap" }}>
        <span>Last run: {state.lastRun ?? "—"}</span>
        <span>Events scraped: {typeof state.lastCount === "number" ? state.lastCount : "—"}</span>
        {state.error && <span style={{ color: "#b91c1c" }}>{state.error}</span>}
      </div>
    </section>
  );
}
