import type { Dispatch, SetStateAction } from "react";
import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import {
  BucketKey,
  PendingEntry,
  bucketKeys,
} from "../../lib/types";
import { usePendingBuckets } from "../hooks/usePendingBuckets";
import { useSettings } from "../hooks/useSettings";

const LABELS: Record<BucketKey, string> = {
  DAY_OF: "Day Of",
  LT_1W: "< 1 Week",
  LT_2W: "< 2 Weeks",
  LT_1M: "< 1 Month",
  LT_2M: "< 2 Months",
  GTE_2M: "≥ 2 Months",
};

type ToastState = {
  kind: "success" | "error";
  message: string;
} | null;

export default function PendingPosts() {
  const { data, loading, refresh } = usePendingBuckets();
  const { settings } = useSettings();
  const [selected, setSelected] = useState<Record<string, boolean>>({});
  const [preview, setPreview] = useState<string>("");
  const [busy, setBusy] = useState(false);
  const [toast, setToast] = useState<ToastState>(null);
  const [previewingId, setPreviewingId] = useState<string | null>(null);
  const [previewEventId, setPreviewEventId] = useState<string | null>(null);
  const [previewCache, setPreviewCache] = useState<Record<string, string>>({});
  const [previewGenre, setPreviewGenre] = useState<string | null>(null);

  const genresById = useMemo(() => {
    const genresMap: Record<string, string | null> = {};
    (bucketKeys as readonly BucketKey[]).forEach((bucket) => {
      data[bucket].forEach(({ event }) => {
        genresMap[event.id] = deriveGenre(event);
      });
    });
    return genresMap;
  }, [data]);

  useEffect(() => {
    if (previewEventId) {
      setPreviewGenre(genresById[previewEventId] ?? null);
    }
  }, [genresById, previewEventId]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const allIds = useMemo(() => {
    return new Set(
      (bucketKeys as readonly BucketKey[]).flatMap((key) =>
        data[key].map((entry) => entry.event.id)
      )
    );
  }, [data]);

  useEffect(() => {
    setSelected((prev) => {
      const next: Record<string, boolean> = {};
      for (const id of Object.keys(prev)) {
        if (allIds.has(id) && prev[id]) {
          next[id] = true;
        }
      }
      return next;
    });
  }, [allIds]);

  const ids = useMemo(
    () => Object.keys(selected).filter((key) => selected[key]),
    [selected]
  );

  async function handlePreview(eventId: string, options?: { force?: boolean }) {
    const force = options?.force ?? false;
    if (!force) {
      const cached = previewCache[eventId];
      if (cached) {
        setPreview(cached);
        setPreviewEventId(eventId);
        setPreviewGenre(genresById[eventId] ?? null);
        setToast(null);
        return;
      }
    }

    try {
      setPreviewingId(eventId);
      const content = await invoke<string>("preview_post", { eventId });
      setPreview(content);
      setPreviewEventId(eventId);
      setPreviewCache((prev) => ({ ...prev, [eventId]: content }));
      setPreviewGenre(genresById[eventId] ?? null);
      setToast({ kind: "success", message: "Preview ready." });
    } catch (error: unknown) {
      console.error(error);
      setToast({ kind: "error", message: "Failed to load preview." });
    } finally {
      setPreviewingId((prev) => (prev === eventId ? null : prev));
    }
  }

  async function handleMarkPosted() {
    if (!ids.length) return;
    setBusy(true);
    setToast(null);
    try {
      await invoke("mark_events_posted", { eventIds: ids });
      setSelected({});
      await refresh();
      if (settings.notifyOnPost) {
        setToast({ kind: "success", message: `Marked ${ids.length} event(s) as posted.` });
      } else {
        setToast(null);
      }
    } catch (error: unknown) {
      console.error(error);
      setToast({ kind: "error", message: "Marking events failed. Check logs." });
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="space-y-6">
      <div className="section-header">
        <div className="section-header__title">Pending Posts</div>
        <div className="actions" style={{ display: "flex", gap: "12px" }}>
          <button className="button" onClick={refresh} disabled={loading || busy}>
            {loading ? "Refreshing…" : "Refresh"}
          </button>
          <button
            className="button"
            disabled={!ids.length || busy}
            onClick={handleMarkPosted}
          >
            {busy ? "Marking…" : `Mark Posted (${ids.length})`}
          </button>
        </div>
      </div>

      <div className="card" style={{ background: "rgba(59, 130, 246, 0.07)" }}>
        <p style={{ margin: 0 }}>
          Copy each draft into Facebook manually. When a show is published, use <strong>Mark Posted</strong>
          to archive it here.
        </p>
      </div>

      {toast && (
        <div className={`toast${toast.kind === "error" ? " toast--error" : ""}`}>
          {toast.message}
        </div>
      )}

      {(bucketKeys as readonly BucketKey[]).map((key) => {
        const rows = data[key];
        if (!rows.length) return null;
        return (
          <BucketSection
            key={key}
            label={LABELS[key]}
            rows={rows}
            selected={selected}
            setSelected={setSelected}
            onPreview={handlePreview}
            previewingId={previewingId}
            previewEventId={previewEventId}
            genresById={genresById}
          />
        );
      })}

      {preview && (
        <div className="card" style={{ whiteSpace: "pre-wrap" }}>
          <div className="badge" style={{ marginBottom: "12px" }}>
            Post Preview
          </div>
          {previewEventId && (
            <div style={{ marginBottom: "12px" }}>
              <button
                className="button"
                onClick={() => handlePreview(previewEventId, { force: true })}
                disabled={!!previewingId}
              >
                {previewingId === previewEventId ? "Refreshing…" : "Regenerate Preview"}
              </button>
            </div>
          )}
          {previewGenre && (
            <div className="badge" style={{ marginBottom: "12px" }}>
              Genre: {previewGenre}
            </div>
          )}
          {preview}
        </div>
      )}
    </div>
  );
}

type BucketSectionProps = {
  label: string;
  rows: PendingEntry[];
  selected: Record<string, boolean>;
  setSelected: Dispatch<SetStateAction<Record<string, boolean>>>;
  onPreview: (eventId: string, options?: { force?: boolean }) => Promise<void>;
  previewingId: string | null;
  previewEventId: string | null;
  genresById: Record<string, string | null>;
};

function BucketSection({
  label,
  rows,
  selected,
  setSelected,
  onPreview,
  previewingId,
  previewEventId,
  genresById,
}: BucketSectionProps) {
  return (
    <section className="card">
      <div className="section-header" style={{ marginBottom: "12px" }}>
        <div className="section-header__title" style={{ fontSize: "18px" }}>
          {label} <span className="badge">{rows.length}</span>
        </div>
      </div>
      <div className="pending-grid">
        {rows.map(({ days_until, event }) => {
          const checked = !!selected[event.id];
          const eventTime = new Date(event.start_local ?? event.start_utc);
          const formatted = eventTime.toLocaleString();
          const isPreviewing = previewingId === event.id;
          const isCurrentPreview = previewEventId === event.id;
          const genre = genresById[event.id];
          return (
            <div key={event.id} className="pending-card">
              <input
                type="checkbox"
                checked={checked}
                onChange={(e) =>
                  setSelected((prev) => ({ ...prev, [event.id]: e.target.checked }))
                }
              />
              <div style={{ flex: 1 }}>
                <div className="pending-card__title">
                  {event.artists[0] ?? "TBA"} — {event.venue_name ?? ""}
                </div>
                <div className="pending-card__meta">
                  {formatted} • {days_until} day{days_until === 1 ? "" : "s"} out
                </div>
                {genre && (
                  <div className="pending-card__meta">
                    Genre: {genre}
                  </div>
                )}
              </div>
              {event.ticket_url && (
                <a
                  className="button"
                  href={event.ticket_url}
                  target="_blank"
                  rel="noreferrer"
                >
                  Tickets
                </a>
              )}
              <button
                className="button"
                onClick={() => onPreview(event.id)}
                disabled={isPreviewing}
              >
                {isPreviewing
                  ? "Previewing…"
                  : isCurrentPreview
                  ? "View Preview"
                  : "Preview"}
              </button>
            </div>
          );
        })}
      </div>
    </section>
  );
}

function deriveGenre(event: PendingEntry["event"]): string | null {
  const tagCandidate = event.tags
    ?.map((tag) => (typeof tag === "string" ? tag.trim() : ""))
    .find((tag) => tag.length > 0);
  if (tagCandidate) {
    return normalizeGenreLabel(tagCandidate);
  }

  if (event.extra && typeof event.extra === "object" && event.extra !== null) {
    const extra = event.extra as Record<string, unknown>;
    const candidates: Array<unknown> = [];
    const possibleKeys = ["genre", "Genre", "style", "Style"]; // fallbacks from scraped data
    possibleKeys.forEach((key) => {
      if (key in extra) {
        candidates.push(extra[key]);
      }
    });
    if ("musicbrainz" in extra) {
      const mbRaw = extra["musicbrainz"];
      if (mbRaw && typeof mbRaw === "object") {
        const mb = mbRaw as Record<string, unknown>;
        if ("genres" in mb) {
          candidates.push(mb["genres"]);
        }
      }
    }
    const extraGenres = extra["genres"];
    if (Array.isArray(extraGenres)) {
      candidates.push(...extraGenres);
    } else if (typeof extraGenres === "string") {
      candidates.push(extraGenres);
    }
    const extraTags = extra["tags"];
    if (Array.isArray(extraTags)) {
      candidates.push(...extraTags);
    }

    for (const candidate of candidates) {
      const value = extractFirstString(candidate);
      if (value) {
        return normalizeGenreLabel(value);
      }
    }
  }

  return null;
}

function extractFirstString(value: unknown): string | null {
  if (typeof value === "string") {
    const trimmed = value.trim();
    return trimmed.length > 0 ? trimmed : null;
  }
  if (Array.isArray(value)) {
    for (const item of value) {
      const result = extractFirstString(item);
      if (result) {
        return result;
      }
    }
  }
  return null;
}

function normalizeGenreLabel(label: string): string {
  return label
    .replace(/[_-]+/g, " ")
    .split(/\s+/)
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}
