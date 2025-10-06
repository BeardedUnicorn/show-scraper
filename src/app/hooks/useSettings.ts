import { useCallback, useEffect, useMemo, useState } from "react";

import type { AppSettings } from "../../lib/types";

const STORAGE_KEY = "show-scrape/settings/v1";

export const DEFAULT_LLM_ENDPOINT = "http://127.0.0.1:1234/v1";

const DEFAULT_SETTINGS: AppSettings = {
  llmModel: "gpt-4o-mini",
  llmEndpoint: DEFAULT_LLM_ENDPOINT,
  dataDirectory: "~/Library/Application Support/show-scrape",
  autoOpenPreview: true,
  notifyOnPost: true,
};

function readSettings(): AppSettings {
  if (typeof window === "undefined") {
    return DEFAULT_SETTINGS;
  }
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) return DEFAULT_SETTINGS;
    const parsed = JSON.parse(raw) as Partial<AppSettings>;
    return { ...DEFAULT_SETTINGS, ...parsed };
  } catch (error) {
    console.warn("Failed to load settings", error);
    return DEFAULT_SETTINGS;
  }
}

export function useSettings() {
  const [settings, setSettings] = useState<AppSettings>(() => ({
    ...DEFAULT_SETTINGS,
  }));
  const [loaded, setLoaded] = useState(false);
  const [dirty, setDirty] = useState(false);
  const [savedAt, setSavedAt] = useState<string | null>(null);

  useEffect(() => {
    if (typeof window === "undefined") return;
    const stored = readSettings();
    setSettings({ ...stored });
    setLoaded(true);
  }, []);

  const update = useCallback(<K extends keyof AppSettings>(key: K, value: AppSettings[K]) => {
    setSettings((prev) => {
      const next = { ...prev, [key]: value };
      return next;
    });
    setDirty(true);
  }, []);

  const reset = useCallback(() => {
    setSettings({ ...DEFAULT_SETTINGS });
    setDirty(true);
  }, []);

  const save = useCallback(() => {
    if (typeof window === "undefined") return;
    try {
      window.localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
      setDirty(false);
      setSavedAt(new Date().toLocaleTimeString());
    } catch (error) {
      console.warn("Failed to persist settings", error);
    }
  }, [settings]);

  const state = useMemo(
    () => ({
      settings,
      loaded,
      dirty,
      savedAt,
      update,
      save,
      reset,
    }),
    [settings, loaded, dirty, savedAt, update, save, reset]
  );

  return state;
}

export function getDefaultSettings(): AppSettings {
  return DEFAULT_SETTINGS;
}
