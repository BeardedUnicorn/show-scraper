import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";

import { DEFAULT_LLM_ENDPOINT, useSettings } from "../hooks/useSettings";

type FacebookStatus = {
  connected: boolean;
  groupId: string | null;
  userName: string | null;
  expiresAt: string | null;
};

type FacebookGroup = {
  id: string;
  name: string;
  administrator: boolean;
};

export default function Settings() {
  const { settings, update, save, reset, loaded, dirty, savedAt } = useSettings();
  const [fbStatus, setFbStatus] = useState<FacebookStatus | null>(null);
  const [fbGroups, setFbGroups] = useState<FacebookGroup[]>([]);
  const [oauthCode, setOauthCode] = useState("");
  const [fbBusy, setFbBusy] = useState(false);
  const [fbError, setFbError] = useState<string | null>(null);
  const [modelOptions, setModelOptions] = useState<string[]>([]);
  const [modelsBusy, setModelsBusy] = useState(false);
  const [modelsError, setModelsError] = useState<string | null>(null);
  const [modelsRefreshKey, setModelsRefreshKey] = useState(0);

  const isDisabled = !loaded;

  const refreshStatus = useCallback(async () => {
    try {
      const status = await invoke<FacebookStatus>("facebook_status");
      setFbStatus(status);
    } catch (error) {
      console.error("Failed to load Facebook status", error);
    }
  }, []);

  useEffect(() => {
    refreshStatus();
  }, [refreshStatus]);

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

  const handleStartOAuth = useCallback(async () => {
    setFbError(null);
    if (!settings.facebookAppId || !settings.facebookRedirectUri) {
      setFbError("Add a Facebook App ID and redirect URI before starting the OAuth flow.");
      return;
    }
    try {
      const url = await invoke<string>("facebook_oauth_url", {
        appId: settings.facebookAppId,
        redirectUri: settings.facebookRedirectUri,
      });
      await openUrl(url);
    } catch (error) {
      console.error(error);
      setFbError("Unable to launch Facebook OAuth; check console for details.");
    }
  }, [settings.facebookAppId, settings.facebookRedirectUri]);

  const handleCompleteOAuth = useCallback(async () => {
    if (!oauthCode.trim()) {
      setFbError("Paste the authorization code returned to your redirect URI.");
      return;
    }
    if (!settings.facebookAppId || !settings.facebookAppSecret || !settings.facebookRedirectUri) {
      setFbError("App ID, App Secret, and redirect URI are required to exchange the code.");
      return;
    }

    try {
      setFbBusy(true);
      setFbError(null);
      const status = await invoke<FacebookStatus>("facebook_complete_oauth", {
        appId: settings.facebookAppId,
        appSecret: settings.facebookAppSecret,
        redirectUri: settings.facebookRedirectUri,
        code: oauthCode.trim(),
      });
      setFbStatus(status);
      setOauthCode("");
      setFbGroups([]);
    } catch (error) {
      console.error(error);
      setFbError("Failed to exchange code for an access token.");
    } finally {
      setFbBusy(false);
    }
  }, [oauthCode, settings.facebookAppId, settings.facebookAppSecret, settings.facebookRedirectUri]);

  const handleLoadGroups = useCallback(async () => {
    try {
      setFbBusy(true);
      setFbError(null);
      const groups = await invoke<FacebookGroup[]>("facebook_list_groups");
      setFbGroups(groups);
    } catch (error) {
      console.error(error);
      setFbError("Unable to load Facebook groups for the connected profile.");
    } finally {
      setFbBusy(false);
    }
  }, []);

  const handleSelectGroup = useCallback(async (groupId: string) => {
    try {
      setFbBusy(true);
      const status = await invoke<FacebookStatus>("facebook_set_group", { groupId });
      setFbStatus(status);
    } catch (error) {
      console.error(error);
      setFbError("Failed to save Facebook group.");
    } finally {
      setFbBusy(false);
    }
  }, []);

  const handleDisconnect = useCallback(async () => {
    try {
      setFbBusy(true);
      await invoke("facebook_disconnect");
      setFbStatus({ connected: false, groupId: null, userName: null, expiresAt: null });
      setFbGroups([]);
    } catch (error) {
      console.error(error);
      setFbError("Failed to disconnect Facebook session.");
    } finally {
      setFbBusy(false);
    }
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
          <h2>Facebook OAuth</h2>
          <p>Sign in with a Facebook app and choose which group receives automated posts.</p>
        </header>

        <div className="form-grid">
          <label className="form-field">
            <span className="form-field__label">Facebook App ID</span>
            <span className="form-field__description">Value from the Facebook developer console.</span>
            <input
              className="input"
              type="text"
              value={settings.facebookAppId}
              onChange={(event) => update("facebookAppId", event.target.value)}
              disabled={isDisabled}
            />
          </label>

          <label className="form-field">
            <span className="form-field__label">Facebook App Secret</span>
            <span className="form-field__description">Only stored locally and used during the code exchange.</span>
            <input
              className="input"
              type="password"
              value={settings.facebookAppSecret}
              onChange={(event) => update("facebookAppSecret", event.target.value)}
              disabled={isDisabled}
            />
          </label>

          <label className="form-field">
            <span className="form-field__label">Redirect URI</span>
            <span className="form-field__description">
              Must match the redirect URI configured in your Facebook app (e.g. `show-scrape://auth/callback`).
            </span>
            <input
              className="input"
              type="text"
              value={settings.facebookRedirectUri}
              onChange={(event) => update("facebookRedirectUri", event.target.value)}
              disabled={isDisabled}
            />
          </label>
        </div>

        <div className="form-grid" style={{ marginTop: 8 }}>
          <div className="form-field">
            <span className="form-field__label">OAuth Actions</span>
            <span className="form-field__description">
              Launch the Facebook login, then paste the returned code to finish the exchange.
            </span>
            <div style={{ display: "flex", gap: "12px", flexWrap: "wrap" }}>
              <button className="button" onClick={handleStartOAuth} disabled={fbBusy || isDisabled}>
                Open Facebook Login
              </button>
              <input
                className="input"
                type="text"
                placeholder="Paste ?code=..."
                value={oauthCode}
                onChange={(event) => setOauthCode(event.target.value)}
                style={{ flex: 1, minWidth: 220 }}
                disabled={fbBusy || isDisabled}
              />
              <button className="button" onClick={handleCompleteOAuth} disabled={fbBusy || isDisabled}>
                Exchange Code
              </button>
              <button className="button" onClick={handleDisconnect} disabled={fbBusy || isDisabled}>
                Disconnect
              </button>
            </div>
          </div>
        </div>

        {fbError && <div className="toast toast--error">{fbError}</div>}

        {fbStatus && (
          <div className="card" style={{ background: "rgba(59, 130, 246, 0.07)" }}>
            <div className="form-field__label">Current Session</div>
            <div className="summary-list" style={{ listStyle: "none", padding: 0 }}>
              <div>Connected: {fbStatus.connected ? "Yes" : "No"}</div>
              <div>User: {fbStatus.userName ?? "—"}</div>
              <div>Group ID: {fbStatus.groupId ?? "Not selected"}</div>
              <div>Token expires: {fbStatus.expiresAt ?? "Unknown"}</div>
            </div>
          </div>
        )}

        <div className="form-grid">
          <div className="form-field">
            <span className="form-field__label">Groups</span>
            <span className="form-field__description">
              Load groups for the connected user and choose the destination for automated posts.
            </span>
            <div style={{ display: "flex", gap: "12px", flexWrap: "wrap" }}>
              <button className="button" onClick={handleLoadGroups} disabled={fbBusy || isDisabled}>
                Load Groups
              </button>
            </div>
            {fbGroups.length > 0 && (
              <div style={{ display: "grid", gap: "8px", marginTop: "12px" }}>
                {fbGroups.map((group) => (
                  <button
                    key={group.id}
                    className="button"
                    style={{
                      justifyContent: "space-between",
                      display: "flex",
                      alignItems: "center",
                    }}
                    onClick={() => handleSelectGroup(group.id)}
                    disabled={fbBusy || isDisabled}
                  >
                    <span>{group.name}</span>
                    <span style={{ fontSize: "12px", opacity: 0.7 }}>
                      {group.administrator ? "Admin" : "Member"}
                    </span>
                  </button>
                ))}
              </div>
            )}
          </div>

          <label className="checkbox-field">
            <input
              type="checkbox"
              checked={settings.notifyOnPost}
              onChange={(event) => update("notifyOnPost", event.target.checked)}
              disabled={isDisabled}
            />
            <div>
              <span className="form-field__label">Show notification after posting</span>
              <span className="form-field__description">
                A toast confirmation keeps humans in the loop after each Graph API call.
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
