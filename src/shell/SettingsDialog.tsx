import { useEffect, useRef, useState } from "react";
import {
  appDir,
  deleteApiKey,
  EFFORTS,
  getSettings,
  hasApiKey,
  setApiKey,
  setSettings,
  type Effort,
  type Provider,
  type Settings,
} from "../api";
import { useT } from "../i18n";

const MODELS = ["claude-opus-5-5", "claude-sonnet-5-5", "claude-fable-5-1", "claude-haiku-4-5"];

/** The MCP bridge next to the app, with the platform's separator and extension. */
function bridgePath(dir: string) {
  return dir.includes("\\") ? `${dir}\\qiyas-mcp.exe` : `${dir}/qiyas-mcp`;
}

/** The commands that connect Claude Code and Claude Desktop to this app's MCP server. */
export function mcpSnippets(dir: string, token: string) {
  const bridge = bridgePath(dir);
  const arg = /\s/.test(bridge) ? `"${bridge}"` : bridge;
  return {
    claudeCode: `claude mcp add qiyas -e QIYAS_MCP_TOKEN=${token} -- ${arg}`,
    claudeDesktop: JSON.stringify(
      { mcpServers: { qiyas: { command: bridge, env: { QIYAS_MCP_TOKEN: token } } } },
      null,
      2,
    ),
  };
}

/** Settings: the AI provider, model, effort and API key, and the MCP server for outside AI apps. */
export function SettingsDialog({ onClose }: { onClose: () => void }) {
  const t = useT();
  const dialog = useRef<HTMLDialogElement>(null);
  const [draft, setDraft] = useState<Settings | null>(null);
  const [dir, setDir] = useState("");
  /** A key is in the keychain for the chosen provider. */
  const [keySaved, setKeySaved] = useState(false);
  /** A new key typed here; empty keeps the one saved. */
  const [key, setKey] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState<string | null>(null);

  useEffect(() => {
    // A modal dialog moves focus inside; give it back to the button that opened it on close.
    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    dialog.current?.showModal();
    getSettings()
      .then(setDraft)
      .catch((e) => setError(String(e)));
    appDir()
      .then(setDir)
      .catch(() => setDir(""));
    return () => opener?.focus();
  }, []);

  const provider = draft?.provider;
  useEffect(() => {
    if (!provider) return;
    setKeySaved(false);
    // No keychain reads as no key; saving one then says why it can't.
    hasApiKey(provider)
      .then(setKeySaved)
      .catch(() => setKeySaved(false));
  }, [provider]);

  const change = (patch: Partial<Settings>) => setDraft((d) => (d ? { ...d, ...patch } : d));

  async function save() {
    if (!draft) return;
    try {
      await setSettings(draft);
      if (key.trim()) await setApiKey(draft.provider, key);
      dialog.current?.close();
    } catch (e) {
      setError(String(e));
    }
  }

  async function removeKey() {
    if (!draft) return;
    try {
      await deleteApiKey(draft.provider);
      setKeySaved(false);
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }

  async function copy(name: string, text: string) {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(name);
    } catch (e) {
      setError(String(e));
    }
  }

  const snippets = draft ? mcpSnippets(dir, draft.mcp_token) : null;

  return (
    <dialog
      ref={dialog}
      className="settings"
      aria-labelledby="settings-title"
      onClose={onClose}
      // Keys typed here are not shortcuts for the drawing behind (Escape still closes the dialog).
      onKeyDown={(e) => e.stopPropagation()}
    >
      <form
        method="dialog"
        onSubmit={(e) => {
          e.preventDefault();
          save();
        }}
      >
        <h2 id="settings-title" className="heading">
          {t("settings")}
        </h2>
        {draft && snippets && (
          <>
            <label>
              <span>{t("settings.provider")}</span>
              <select
                value={draft.provider}
                onChange={(e) => change({ provider: e.target.value as Provider })}
              >
                <option value="anthropic">{t("settings.providerAnthropic")}</option>
                <option value="openai_compatible">{t("settings.providerOpenai")}</option>
              </select>
            </label>
            <label>
              <span>{t("settings.model")}</span>
              <input
                dir="ltr"
                list="settings-models"
                value={draft.model}
                onChange={(e) => change({ model: e.target.value })}
              />
              <datalist id="settings-models">
                {MODELS.map((m) => (
                  <option key={m} value={m} />
                ))}
              </datalist>
            </label>
            {draft.provider === "openai_compatible" && (
              <label>
                <span>{t("settings.baseUrl")}</span>
                <input
                  dir="ltr"
                  type="url"
                  placeholder="http://localhost:11434/v1"
                  value={draft.base_url ?? ""}
                  onChange={(e) => change({ base_url: e.target.value || null })}
                />
              </label>
            )}
            <label>
              <span>{t("settings.effort")}</span>
              <select
                value={draft.effort}
                onChange={(e) => change({ effort: e.target.value as Effort })}
              >
                {EFFORTS.map((x) => (
                  <option key={x} value={x}>
                    {t(`effort.${x}`)}
                  </option>
                ))}
              </select>
            </label>
            <div className="field">
              <span>
                <label htmlFor="settings-key">{t("settings.apiKey")}</label>
                {keySaved && <span className="badge">{t("settings.saved")}</span>}
              </span>
              <span className="row">
                <input
                  id="settings-key"
                  dir="ltr"
                  type="password"
                  autoComplete="off"
                  placeholder={keySaved ? "••••••••" : ""}
                  value={key}
                  onChange={(e) => setKey(e.target.value)}
                />
                {keySaved && (
                  <button type="button" onClick={removeKey}>
                    {t("settings.remove")}
                  </button>
                )}
              </span>
            </div>

            <fieldset>
              <legend className="heading">{t("settings.mcp")}</legend>
              <label className="check">
                <input
                  type="checkbox"
                  checked={draft.mcp_enabled}
                  onChange={(e) => change({ mcp_enabled: e.target.checked })}
                />
                <span>{t("settings.mcpEnable")}</span>
              </label>
              <label>
                <span>{t("settings.port")}</span>
                <input
                  dir="ltr"
                  type="number"
                  min={1024}
                  max={65535}
                  value={draft.mcp_port}
                  onChange={(e) => change({ mcp_port: Number(e.target.value) })}
                />
              </label>
              <div className="field">
                <label htmlFor="settings-token">{t("settings.token")}</label>
                <span className="row">
                  <input id="settings-token" dir="ltr" readOnly value={draft.mcp_token} />
                  <button type="button" onClick={() => copy("token", draft.mcp_token)}>
                    {copied === "token" ? t("settings.copied") : t("settings.copy")}
                  </button>
                </span>
              </div>
              <div className="snippet">
                <span>{t("settings.claudeCode")}</span>
                <pre dir="ltr">{snippets.claudeCode}</pre>
                <button type="button" onClick={() => copy("code", snippets.claudeCode)}>
                  {copied === "code" ? t("settings.copied") : t("settings.copy")}
                </button>
              </div>
              <div className="snippet">
                <span>{t("settings.claudeDesktop")}</span>
                <pre dir="ltr">{snippets.claudeDesktop}</pre>
                <button type="button" onClick={() => copy("desktop", snippets.claudeDesktop)}>
                  {copied === "desktop" ? t("settings.copied") : t("settings.copy")}
                </button>
              </div>
            </fieldset>
          </>
        )}
        {error && (
          <p className="error" role="alert">
            {error}
          </p>
        )}
        <div className="buttons">
          <button type="button" onClick={() => dialog.current?.close()}>
            {t("settings.cancel")}
          </button>
          <button type="submit" className="primary" disabled={!draft}>
            {t("settings.save")}
          </button>
        </div>
      </form>
    </dialog>
  );
}
