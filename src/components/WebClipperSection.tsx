import { useEffect, useState } from "react";
import { Globe, RefreshCcw, Copy, Check } from "lucide-react";
import { api } from "../api";
import { useStore } from "../store";

/**
 * v0.24.0 — Settings → Web Clipper.
 *
 * Surfaces the localhost port + bearer token the user pastes into
 * their browser extension's Options page. Token regeneration
 * invalidates any prior pairing (extension must be re-paired).
 *
 * Why manual paste and not auto-discovery: any mDNS / well-known-port
 * probe is exploitable by malicious local processes. Manual copy-
 * paste is 15 seconds and bulletproof. See the comment block at the
 * top of `src-tauri/src/web_clipper.rs` for the threat model.
 */
export function WebClipperSection() {
  const [port, setPort] = useState<number | null>(null);
  const [token, setToken] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [copiedField, setCopiedField] = useState<"port" | "token" | null>(null);
  const showToast = useStore((s) => s.showToast);

  const refresh = async () => {
    try {
      const info = await api.getWebClipperInfo();
      setPort(info.port);
      setToken(info.token);
    } catch (e) {
      showToast("Could not read Web Clipper info: " + String(e));
    }
  };

  useEffect(() => {
    void refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const copy = async (text: string, which: "port" | "token") => {
    try {
      await navigator.clipboard.writeText(text);
      setCopiedField(which);
      window.setTimeout(() => setCopiedField(null), 1500);
    } catch {
      showToast("Copy failed — select + Ctrl+C manually");
    }
  };

  const onRegenerate = async () => {
    if (!confirm("Regenerate token? Any paired browser extensions will need the new token.")) return;
    setBusy(true);
    try {
      const t = await api.regenerateWebClipperToken();
      setToken(t);
      showToast("Token regenerated — re-pair your extension");
    } catch (e) {
      showToast("Could not regenerate token: " + String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div>
      <div className="font-medium flex items-center gap-2">
        <Globe size={16} aria-hidden /> Web Clipper
      </div>
      <p className="text-sm text-gray-600 dark:text-gray-400 mt-1">
        Save the page you're reading straight into Keepr from your browser.
        Install the Keepr extension (Chrome / Firefox / Edge), open its
        Options page, and paste these two values. The connection is
        localhost-only — nothing leaves your machine.
      </p>

      {port === null || token === null ? (
        <div className="mt-3 text-sm text-gray-500 dark:text-gray-400">
          Starting local server…
        </div>
      ) : (
        <div className="mt-3 space-y-3">
          <Field
            label="Port"
            value={String(port)}
            copied={copiedField === "port"}
            onCopy={() => copy(String(port), "port")}
          />
          <Field
            label="Bearer token"
            value={token}
            mono
            copied={copiedField === "token"}
            onCopy={() => copy(token, "token")}
          />
          <div className="flex flex-wrap items-center gap-2">
            <button
              type="button"
              onClick={onRegenerate}
              disabled={busy}
              className="flex items-center gap-2 px-3 py-2 text-sm rounded border border-gray-300 dark:border-[#5f6368] hover:bg-black/5 dark:hover:bg-white/10 disabled:opacity-50"
            >
              <RefreshCcw size={14} aria-hidden /> Regenerate token
            </button>
            <span className="text-[11px] text-gray-500 dark:text-gray-400">
              Endpoint:{" "}
              <span className="font-mono">http://127.0.0.1:{port}/clip</span>
            </span>
          </div>
        </div>
      )}
    </div>
  );
}

function Field({
  label,
  value,
  mono,
  copied,
  onCopy,
}: {
  label: string;
  value: string;
  mono?: boolean;
  copied: boolean;
  onCopy: () => void;
}) {
  return (
    <div>
      <div className="text-[11px] uppercase tracking-wide text-gray-500 dark:text-gray-400">
        {label}
      </div>
      <div className="mt-1 flex items-center gap-2">
        <code
          className={
            "flex-1 px-2 py-1.5 text-xs rounded bg-black/5 dark:bg-white/10 break-all select-all " +
            (mono ? "font-mono" : "")
          }
        >
          {value}
        </code>
        <button
          type="button"
          onClick={onCopy}
          aria-label={`Copy ${label}`}
          className="p-1.5 rounded hover:bg-black/5 dark:hover:bg-white/10"
        >
          {copied ? (
            <Check size={14} className="text-green-600 dark:text-green-400" aria-hidden />
          ) : (
            <Copy size={14} aria-hidden />
          )}
        </button>
      </div>
    </div>
  );
}
