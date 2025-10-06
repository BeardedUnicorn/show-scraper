import { useCallback, useEffect, useMemo, useState } from "react";

import { DEFAULT_LLM_ENDPOINT, useSettings } from "../hooks/useSettings";

export default function Settings() {
  const { settings, update, save, reset, loaded, dirty, savedAt } = useSettings();
  const [modelOptions, setModelOptions] = useState<string[]>([]);
  const [modelsBusy, setModelsBusy] = useState(false);
  const [modelsError, setModelsError] = useState<string | null>(null);
  const [modelsRefreshKey, setModelsRefreshKey] = useState(0);

  const isDisabled = !loaded;

  const effectiveLlmEndpoint = useMemo(() => {
    const trimmed = settings.llmEndpoint.trim();
    return trimmed || DEFAULT_LLM_ENDPOINT;
  }, [settings.llmEndpoint]);

  const modelSelectOptions = useMemo(() => {
    const seen = new Set<string>();
    const ordered: string[] = [];
    for (const value of modelOptions) {
      if (value && !seen.has(value)) {
        seen.add(value);
        ordered.push(value);
      }
    }
    if (settings.llmModel && !seen.has(settings.llmModel)) {
      ordered.unshift(settings.llmModel);
    }
    return ordered;
  }, [modelOptions, settings.llmModel]);

  useEffect(() => {
    if (!loaded || typeof window === "undefined") return;
    const controller = new AbortController();
    let active = true;

    const fetchModels = async () => {
      setModelsBusy(true);
      setModelsError(null);

      const normalized = effectiveLlmEndpoint.replace(/\/$/, "");
      const modelsUrl = `${normalized}/models`;

      try {
        const response = await fetch(modelsUrl, {
          signal: controller.signal,
          headers: { Accept: "application/json" },
        });
        if (!response.ok) {
          throw new Error(`HTTP ${response.status} ${response.statusText}`.trim());
        }
        const payload = await response.json();
        const values: string[] = [];
        const push = (entry: unknown) => {
          if (typeof entry === "string") {
            values.push(entry);
            return;
          }
          if (entry && typeof entry === "object" && "id" in entry) {
            const id = (entry as { id?: unknown }).id;
            if (typeof id === "string") {
              values.push(id);
              return;
            }
          }
          if (entry && typeof entry === "object" && "model" in entry) {
            const model = (entry as { model?: unknown }).model;
            if (typeof model === "string") {
              values.push(model);
            }
          }
        };

        if (Array.isArray(payload)) {
          payload.forEach(push);
        } else if (payload && typeof payload === "object") {
          if (Array.isArray((payload as { data?: unknown }).data)) {
            ((payload as { data: unknown[] }).data).forEach(push);
          } else if (Array.isArray((payload as { models?: unknown }).models)) {
            ((payload as { models: unknown[] }).models).forEach(push);
          }
        }

        const unique = Array.from(new Set(values.filter((value) => value && value.trim().length > 0)));
        if (active) {
          setModelOptions(unique);
          if (!unique.length) {
            setModelsError("No models returned by the API.");
          }
        }
      } catch (error) {
        if (!active || (error instanceof DOMException && error.name === "AbortError")) {
          return;
        }
        setModelOptions([]);
        setModelsError(
          error instanceof Error
            ? `Unable to load models: ${error.message}`
            : "Unable to load models."
        );
      } finally {
        if (active) {
          setModelsBusy(false);
        }
      }
    };

    const timeout = window.setTimeout(fetchModels, 250);

    return () => {
      active = false;
      controller.abort();
      window.clearTimeout(timeout);
    };
  }, [effectiveLlmEndpoint, loaded, modelsRefreshKey]);

  const handleReloadModels = useCallback(() => {
    setModelsRefreshKey((prev) => prev + 1);
  }, []);

  return (
    <div className="space-y-6">
      <div className="section-header">
        <div className="section-header__title">Settings</div>
        <div className="actions" style={{ display: "flex", gap: "12px" }}>
          <button className="button" onClick={reset} disabled={isDisabled}>
            Reset to Defaults
          </button>
          <button
            className="button"
            onClick={save}
            disabled={isDisabled || !dirty}
          >
            Save Changes
          </button>
        </div>
      </div>

      <div className="card form-section">
        <header className="form-section__header">
          <h2>AI Composer</h2>
          <p>Configure how the desktop app talks to your OpenAI-compatible server.</p>
        </header>
        <div className="form-grid">
          <label className="form-field">
            <span className="form-field__label">OpenAI-compatible Endpoint</span>
            <span className="form-field__description">
              Defaults to {DEFAULT_LLM_ENDPOINT}. Update this to point at your server’s `/v1` base URL.
            </span>
            <input
              className="input"
              type="text"
              value={settings.llmEndpoint}
              onChange={(event) => update("llmEndpoint", event.target.value)}
              placeholder={DEFAULT_LLM_ENDPOINT}
              disabled={isDisabled}
            />
          </label>

          <label className="form-field">
            <span className="form-field__label">Model</span>
            <span className="form-field__description">
              Pick from models reported by the API’s `/models` endpoint.
            </span>
            <div style={{ display: "flex", gap: "12px", flexWrap: "wrap" }}>
              <select
                className="input"
                value={settings.llmModel || ""}
                onChange={(event) => update("llmModel", event.target.value)}
                disabled={isDisabled || modelsBusy || !modelSelectOptions.length}
                style={{ flex: "1 1 220px", minWidth: "160px" }}
              >
                <option value="" disabled>
                  {modelsBusy
                    ? "Loading models…"
                    : modelSelectOptions.length
                    ? "Select a model"
                    : "No models available"}
                </option>
                {modelSelectOptions.map((value) => (
                  <option key={value} value={value}>
                    {value}
                  </option>
                ))}
              </select>
              <button
                className="button"
                type="button"
                onClick={handleReloadModels}
                disabled={modelsBusy || isDisabled}
              >
                {modelsBusy ? "Refreshing…" : "Reload Models"}
              </button>
            </div>
            {modelsError && (
              <div className="form-field__description" style={{ color: "#c00" }}>
                {modelsError}
              </div>
            )}
          </label>
        </div>
      </div>

      <div className="card form-section">
        <header className="form-section__header">
          <h2>Posting Workflow</h2>
          <p>Drafts are generated here and posted to Facebook manually.</p>
        </header>

        <div className="form-grid">
          <label className="checkbox-field">
            <input
              type="checkbox"
              checked={settings.notifyOnPost}
              onChange={(event) => update("notifyOnPost", event.target.checked)}
              disabled={isDisabled}
            />
            <div>
              <span className="form-field__label">Show notification after marking posted</span>
              <span className="form-field__description">
                Keep the manual workflow visible with a toast after events are marked complete.
              </span>
            </div>
          </label>

          <label className="checkbox-field">
            <input
              type="checkbox"
              checked={settings.autoOpenPreview}
              onChange={(event) => update("autoOpenPreview", event.target.checked)}
              disabled={isDisabled}
            />
            <div>
              <span className="form-field__label">Auto-open post preview</span>
              <span className="form-field__description">
                When enabled, the Pending Posts screen shows the AI draft after a scrape finishes.
              </span>
            </div>
          </label>
        </div>
      </div>

      <div className="card form-section">
        <header className="form-section__header">
          <h2>Storage</h2>
          <p>Control where exports, logs, and the SQLite database live.</p>
        </header>
        <div className="form-grid">
          <label className="form-field">
            <span className="form-field__label">Data Directory</span>
            <span className="form-field__description">
              Set the folder where the scraper writes the SQLite database and debug output.
            </span>
            <input
              className="input"
              type="text"
              value={settings.dataDirectory}
              onChange={(event) => update("dataDirectory", event.target.value)}
              disabled={isDisabled}
            />
          </label>
        </div>
      </div>

      <div className="card form-section">
        <header className="form-section__header">
          <h2>Quick Reference</h2>
          <p>All settings are stored locally on this machine.</p>
        </header>
        <ul className="summary-list">
          <li>
            <strong>Save</strong> commits changes to browser storage and updates the Pending Posts behavior.
          </li>
          <li>
            <strong>Reset</strong> restores defaults (model, directories, toggles) without touching scraped data.
          </li>
          <li>{savedAt ? `Last saved ${savedAt}.` : "Settings have not been saved yet."}</li>
        </ul>
      </div>
    </div>
  );
}
