import { useEffect, useRef, useState } from "react";
import { Lock, Eye, EyeOff } from "lucide-react";
import { api } from "../api";
import { useStore } from "../store";

/**
 * NF-V0.5-C App Lock — full-screen overlay shown when `store.locked`
 * is true. Argon2id verification happens in Rust (~150-300 ms), so
 * the Unlock button shows a busy state during the call. We deliberately
 * don't rate-limit attempts in the renderer: the slow KDF already costs
 * an attacker ~3-7 attempts/second, and rate-limiting in the UI is
 * trivially bypassed by anyone with access to the on-disk SQLite file.
 */
export function LockScreen() {
  const locked = useStore((s) => s.locked);
  const unlock = useStore((s) => s.unlock);

  const [pin, setPin] = useState("");
  const [show, setShow] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (locked) {
      setPin("");
      setError(null);
      inputRef.current?.focus();
    }
  }, [locked]);

  if (!locked) return null;

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (busy || pin.length === 0) return;
    setBusy(true);
    setError(null);
    try {
      const ok = await api.verifyAppLockPin(pin);
      if (ok) {
        unlock();
      } else {
        setError("Incorrect PIN");
        setPin("");
        inputRef.current?.focus();
      }
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div
      className="fixed inset-0 z-[100] grid place-items-center bg-white dark:bg-[#202124] text-gray-800 dark:text-gray-100"
      role="dialog"
      aria-modal="true"
      aria-labelledby="lock-screen-title"
    >
      <div className="w-full max-w-sm px-6 py-10 text-center">
        <div className="mx-auto mb-6 w-16 h-16 rounded-full bg-[#fdd663] dark:bg-[#41331c] grid place-items-center">
          <Lock size={28} className="text-[#202124] dark:text-[#fdd663]" aria-hidden />
        </div>
        <h1 id="lock-screen-title" className="text-2xl font-medium mb-2">
          Keepr is locked
        </h1>
        <p className="text-sm text-gray-500 dark:text-gray-400 mb-6">
          Enter your PIN to continue.
        </p>
        <form onSubmit={submit} className="space-y-3">
          <div className="relative">
            <input
              ref={inputRef}
              type={show ? "text" : "password"}
              value={pin}
              onChange={(e) => setPin(e.target.value)}
              autoFocus
              autoComplete="current-password"
              aria-label="App Lock PIN"
              className="w-full px-3 py-2 pr-10 text-base rounded border border-gray-300 dark:border-[#5f6368] bg-transparent focus:outline-none focus:ring-2 focus:ring-[#1a73e8]"
              disabled={busy}
            />
            <button
              type="button"
              onClick={() => setShow((v) => !v)}
              aria-label={show ? "Hide PIN" : "Show PIN"}
              className="absolute top-1/2 right-2 -translate-y-1/2 p-1 rounded hover:bg-black/5 dark:hover:bg-white/10"
              tabIndex={-1}
            >
              {show ? <EyeOff size={16} aria-hidden /> : <Eye size={16} aria-hidden />}
            </button>
          </div>
          {error && (
            <div
              role="alert"
              className="text-sm text-[#d93025]"
            >
              {error}
            </div>
          )}
          <button
            type="submit"
            disabled={busy || pin.length === 0}
            className="w-full px-4 py-2 rounded bg-[#1a73e8] text-white font-medium hover:bg-[#1557b0] disabled:opacity-50"
          >
            {busy ? "Verifying…" : "Unlock"}
          </button>
        </form>
        <p className="text-xs text-gray-500 dark:text-gray-400 mt-6">
          Forgot your PIN? There is no recovery. See SECURITY.md.
        </p>
      </div>
    </div>
  );
}
