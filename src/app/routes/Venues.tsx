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
    <div className="space-y-6" data-testid="venues-page">
      <div className="section-header" data-testid="venues-header">
        <div className="section-header__title" data-testid="venues-title">
          Venues
        </div>
        <div
          className="actions"
          style={{ display: "flex", gap: "12px" }}
          data-testid="venues-actions"
        >
          <button
            className="button"
            onClick={loadVenues}
            disabled={loading || bulkBusy}
            data-testid="venues-refresh-button"
          >
            {loading ? "Refreshing…" : "Refresh"}
          </button>
          <button
            className="button"
            onClick={handleScrapeAll}
            disabled={bulkBusy || loading}
            data-testid="venues-scrape-all-button"
          >
            {bulkBusy ? "Scraping…" : "Scrape All"}
          </button>
        </div>
      </div>

      {toast && (
        <div
          className={`toast${toast.kind === "error" ? " toast--error" : ""}`}
          data-testid="venues-toast"
        >
          {toast.message}
        </div>
      )}

      {loading && !venues.length ? (
        <div className="card" data-testid="venues-loading">Loading venues…</div>
      ) : !sortedVenues.length ? (
        <div className="card" data-testid="venues-empty">No venues configured yet.</div>
      ) : (
        <div className="pending-grid" data-testid="venues-grid">
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
  const testIdSlug = venue.id
    ? venue.id.toString().toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/(^-|-$)/g, "")
    : "venue";
  return (
    <section
      className="card"
      style={{ display: "flex", flexDirection: "column", gap: "12px" }}
      data-testid={`venue-card-${testIdSlug}`}
    >
      <div
        style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start" }}
        data-testid={`venue-card-header-${testIdSlug}`}
      >
        <div data-testid={`venue-card-details-${testIdSlug}`}>
          <div
            className="pending-card__title"
            style={{ fontSize: "18px" }}
            data-testid={`venue-card-name-${testIdSlug}`}
          >
            {venue.name}
          </div>
          <a
            href={venue.url}
            className="pending-card__meta"
            target="_blank"
            rel="noreferrer"
            data-testid={`venue-card-url-${testIdSlug}`}
          >
            {venue.url}
          </a>
        </div>
        <button
          className="button"
          disabled={busy}
          onClick={() => onScrape(venue.id)}
          style={{ minWidth: "120px", justifyContent: "center" }}
          data-testid={`venue-card-scrape-button-${testIdSlug}`}
        >
          {state.running ? "Scraping…" : "Run Scrape"}
        </button>
      </div>
      <div
        className="pending-card__meta"
        style={{ display: "flex", gap: "16px", flexWrap: "wrap" }}
        data-testid={`venue-card-meta-${testIdSlug}`}
      >
        <span data-testid={`venue-card-last-run-${testIdSlug}`}>
          Last run: {state.lastRun ?? "—"}
        </span>
        <span data-testid={`venue-card-last-count-${testIdSlug}`}>
          Events scraped: {typeof state.lastCount === "number" ? state.lastCount : "—"}
        </span>
        {state.error && (
          <span style={{ color: "#b91c1c" }} data-testid={`venue-card-error-${testIdSlug}`}>
            {state.error}
          </span>
        )}
      </div>
    </section>
  );
}
