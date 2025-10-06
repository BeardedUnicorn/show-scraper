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
    <div className="space-y-6" data-testid="settings-page">
      <div className="section-header" data-testid="settings-header">
        <div className="section-header__title" data-testid="settings-title">
          Settings
        </div>
        <div
          className="actions"
          style={{ display: "flex", gap: "12px" }}
          data-testid="settings-header-actions"
        >
          <button
            className="button"
            onClick={reset}
            disabled={isDisabled}
            data-testid="settings-reset-button"
          >
            Reset to Defaults
          </button>
          <button
            className="button"
            onClick={save}
            disabled={isDisabled || !dirty}
            data-testid="settings-save-button"
          >
            Save Changes
          </button>
        </div>
      </div>

      <div className="card form-section" data-testid="settings-ai-composer-section">
        <header className="form-section__header" data-testid="settings-ai-composer-header">
          <h2 data-testid="settings-ai-composer-title">AI Composer</h2>
          <p data-testid="settings-ai-composer-description">
            Configure how the desktop app talks to your OpenAI-compatible server.
          </p>
        </header>
        <div className="form-grid" data-testid="settings-ai-composer-grid">
          <label className="form-field" data-testid="settings-endpoint-field">
            <span className="form-field__label" data-testid="settings-endpoint-label">
              OpenAI-compatible Endpoint
            </span>
            <span
              className="form-field__description"
              data-testid="settings-endpoint-description"
            >
              Defaults to {DEFAULT_LLM_ENDPOINT}. Update this to point at your server’s `/v1` base URL.
            </span>
            <input
              className="input"
              type="text"
              value={settings.llmEndpoint}
              onChange={(event) => update("llmEndpoint", event.target.value)}
              placeholder={DEFAULT_LLM_ENDPOINT}
              disabled={isDisabled}
              data-testid="settings-endpoint-input"
            />
          </label>

          <label className="form-field" data-testid="settings-model-field">
            <span className="form-field__label" data-testid="settings-model-label">
              Model
            </span>
            <span className="form-field__description" data-testid="settings-model-description">
              Pick from models reported by the API’s `/models` endpoint.
            </span>
            <div
              style={{ display: "flex", gap: "12px", flexWrap: "wrap" }}
              data-testid="settings-model-controls"
            >
              <select
                className="input"
                value={settings.llmModel || ""}
                onChange={(event) => update("llmModel", event.target.value)}
                disabled={isDisabled || modelsBusy || !modelSelectOptions.length}
                style={{ flex: "1 1 220px", minWidth: "160px" }}
                data-testid="settings-model-select"
              >
                <option value="" disabled>
                  {modelsBusy
                    ? "Loading models…"
                    : modelSelectOptions.length
                    ? "Select a model"
                    : "No models available"}
                </option>
                {modelSelectOptions.map((value) => (
                  <option key={value} value={value} data-testid={`settings-model-option-${value}`}>
                    {value}
                  </option>
                ))}
              </select>
              <button
                className="button"
                type="button"
                onClick={handleReloadModels}
                disabled={modelsBusy || isDisabled}
                data-testid="settings-reload-models-button"
              >
                {modelsBusy ? "Refreshing…" : "Reload Models"}
              </button>
            </div>
            {modelsError && (
              <div
                className="form-field__description"
                style={{ color: "#c00" }}
                data-testid="settings-model-error"
              >
                {modelsError}
              </div>
            )}
          </label>
        </div>
      </div>

      <div className="card form-section" data-testid="settings-posting-section">
        <header className="form-section__header" data-testid="settings-posting-header">
          <h2 data-testid="settings-posting-title">Posting Workflow</h2>
          <p data-testid="settings-posting-description">
            Drafts are generated here and posted to Facebook manually.
          </p>
        </header>

        <div className="form-grid" data-testid="settings-posting-grid">
          <label className="checkbox-field" data-testid="settings-notify-field">
            <input
              type="checkbox"
              checked={settings.notifyOnPost}
              onChange={(event) => update("notifyOnPost", event.target.checked)}
              disabled={isDisabled}
              data-testid="settings-notify-checkbox"
            />
            <div data-testid="settings-notify-text">
              <span className="form-field__label" data-testid="settings-notify-label">
                Show notification after marking posted
              </span>
              <span
                className="form-field__description"
                data-testid="settings-notify-description"
              >
                Keep the manual workflow visible with a toast after events are marked complete.
              </span>
            </div>
          </label>

          <label className="checkbox-field" data-testid="settings-auto-preview-field">
            <input
              type="checkbox"
              checked={settings.autoOpenPreview}
              onChange={(event) => update("autoOpenPreview", event.target.checked)}
              disabled={isDisabled}
              data-testid="settings-auto-preview-checkbox"
            />
            <div data-testid="settings-auto-preview-text">
              <span className="form-field__label" data-testid="settings-auto-preview-label">
                Auto-open post preview
              </span>
              <span
                className="form-field__description"
                data-testid="settings-auto-preview-description"
              >
                When enabled, the Pending Posts screen shows the AI draft after a scrape finishes.
              </span>
            </div>
          </label>
        </div>
      </div>

      <div className="card form-section" data-testid="settings-storage-section">
        <header className="form-section__header" data-testid="settings-storage-header">
          <h2 data-testid="settings-storage-title">Storage</h2>
          <p data-testid="settings-storage-description">
            Control where exports, logs, and the SQLite database live.
          </p>
        </header>
        <div className="form-grid" data-testid="settings-storage-grid">
          <label className="form-field" data-testid="settings-data-directory-field">
            <span className="form-field__label" data-testid="settings-data-directory-label">
              Data Directory
            </span>
            <span
              className="form-field__description"
              data-testid="settings-data-directory-description"
            >
              Set the folder where the scraper writes the SQLite database and debug output.
            </span>
            <input
              className="input"
              type="text"
              value={settings.dataDirectory}
              onChange={(event) => update("dataDirectory", event.target.value)}
              disabled={isDisabled}
              data-testid="settings-data-directory-input"
            />
          </label>
        </div>
      </div>

      <div className="card form-section" data-testid="settings-quick-reference-section">
        <header className="form-section__header" data-testid="settings-quick-reference-header">
          <h2 data-testid="settings-quick-reference-title">Quick Reference</h2>
          <p data-testid="settings-quick-reference-description">
            All settings are stored locally on this machine.
          </p>
        </header>
        <ul className="summary-list" data-testid="settings-quick-reference-list">
          <li data-testid="settings-quick-reference-save">
            <strong>Save</strong> commits changes to browser storage and updates the Pending Posts behavior.
          </li>
          <li data-testid="settings-quick-reference-reset">
            <strong>Reset</strong> restores defaults (model, directories, toggles) without touching scraped data.
          </li>
          <li data-testid="settings-quick-reference-saved-at">
            {savedAt ? `Last saved ${savedAt}.` : "Settings have not been saved yet."}
          </li>
        </ul>
      </div>
    </div>
  );
}
