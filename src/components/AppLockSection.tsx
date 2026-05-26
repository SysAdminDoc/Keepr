import { useState } from "react";
import { Lock, Unlock } from "lucide-react";
import { useStore } from "../store";
import { api } from "../api";

/**
 * NF-V0.5-C Settings → App Lock section. Three states:
 *   1. Not configured → Enable form (PIN + Confirm + minutes).
 *   2. Configured     → Change minutes slider + Disable form (current PIN).
 *   3. Busy           → buttons disabled while Argon2id chews.
 */
export function AppLockSection() {
  const appLockEnabled = useStore((s) => s.appLockEnabled);
  const lockAfterMinutes = useStore((s) => s.lockAfterMinutes);
  const refresh = useStore((s) => s.refreshAppLockState);
  const showToast = useStore((s) => s.showToast);
  const lock = useStore((s) => s.lock);

  return (
    <div>
      <div className="font-medium flex items-center gap-2">
        <Lock size={16} aria-hidden /> App Lock
      </div>
      <p className="text-sm text-gray-600 dark:text-gray-400 mt-1">
        Hides every note behind a PIN whenever you launch Keepr or step
        away for a while. The PIN never leaves your machine. App Lock is
        a UI gate only — see <code>SECURITY.md</code> for what it doesn't
        protect against, and note that there is no PIN recovery.
      </p>
      {appLockEnabled ? (
        <ConfiguredPanel
          lockAfterMinutes={lockAfterMinutes}
          onChange={async (minutes) => {
            try {
              await api.setAppLockMinutes(minutes);
              await refresh();
            } catch (e) {
              showToast("Could not update lock timeout: " + String(e));
            }
          }}
          onLockNow={() => lock()}
          onDisabled={refresh}
        />
      ) : (
        <EnablePanel onEnabled={refresh} />
      )}
    </div>
  );
}

function EnablePanel({ onEnabled }: { onEnabled: () => Promise<void> }) {
  const showToast = useStore((s) => s.showToast);
  const [pin, setPin] = useState("");
  const [confirm, setConfirm] = useState("");
  const [minutes, setMinutes] = useState(5);
  const [busy, setBusy] = useState(false);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (busy) return;
    if (pin.length < 4) {
      showToast("PIN must be at least 4 characters");
      return;
    }
    if (pin !== confirm) {
      showToast("PINs don't match");
      return;
    }
    setBusy(true);
    try {
      await api.enableAppLock(pin, minutes);
      await onEnabled();
      setPin("");
      setConfirm("");
      showToast("App Lock enabled");
    } catch (err) {
      showToast("Could not enable App Lock: " + String(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <form onSubmit={submit} className="mt-3 grid gap-2 sm:grid-cols-2">
      <input
        type="password"
        value={pin}
        onChange={(e) => setPin(e.target.value)}
        placeholder="New PIN (min 4 chars)"
        aria-label="New App Lock PIN"
        autoComplete="new-password"
        className="px-3 py-1.5 text-sm rounded border border-gray-300 dark:border-[#5f6368] bg-transparent"
        disabled={busy}
      />
      <input
        type="password"
        value={confirm}
        onChange={(e) => setConfirm(e.target.value)}
        placeholder="Confirm PIN"
        aria-label="Confirm App Lock PIN"
        autoComplete="new-password"
        className="px-3 py-1.5 text-sm rounded border border-gray-300 dark:border-[#5f6368] bg-transparent"
        disabled={busy}
      />
      <label className="text-sm flex items-center gap-2 sm:col-span-2">
        Lock after
        <select
          value={minutes}
          onChange={(e) => setMinutes(parseInt(e.target.value, 10))}
          aria-label="Lock after how many minutes of inactivity"
          className="px-2 py-1 text-sm rounded border border-gray-300 dark:border-[#5f6368] bg-transparent"
          disabled={busy}
        >
          <option value={1}>1 minute</option>
          <option value={5}>5 minutes</option>
          <option value={15}>15 minutes</option>
          <option value={30}>30 minutes</option>
          <option value={60}>1 hour</option>
        </select>
        of inactivity.
      </label>
      <button
        type="submit"
        disabled={busy}
        className="sm:col-span-2 px-3 py-2 text-sm rounded bg-[var(--keepr-accent)] text-white font-medium hover:bg-[var(--keepr-accent-hover)] disabled:opacity-50"
      >
        {busy ? "Hashing…" : "Enable App Lock"}
      </button>
    </form>
  );
}

function ConfiguredPanel({
  lockAfterMinutes,
  onChange,
  onLockNow,
  onDisabled,
}: {
  lockAfterMinutes: number;
  onChange: (minutes: number) => void;
  onLockNow: () => void;
  onDisabled: () => Promise<void>;
}) {
  const showToast = useStore((s) => s.showToast);
  const [currentPin, setCurrentPin] = useState("");
  const [busy, setBusy] = useState(false);

  const disable = async (e: React.FormEvent) => {
    e.preventDefault();
    if (busy || currentPin.length === 0) return;
    setBusy(true);
    try {
      await api.disableAppLock(currentPin);
      await onDisabled();
      setCurrentPin("");
      showToast("App Lock disabled");
    } catch (err) {
      showToast(String(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="mt-3 space-y-3">
      <label className="text-sm flex items-center gap-2 flex-wrap">
        Lock after
        <select
          value={lockAfterMinutes}
          onChange={(e) => onChange(parseInt(e.target.value, 10))}
          aria-label="Lock after how many minutes of inactivity"
          className="px-2 py-1 text-sm rounded border border-gray-300 dark:border-[#5f6368] bg-transparent"
        >
          <option value={1}>1 minute</option>
          <option value={5}>5 minutes</option>
          <option value={15}>15 minutes</option>
          <option value={30}>30 minutes</option>
          <option value={60}>1 hour</option>
        </select>
        of inactivity.
      </label>
      <button
        type="button"
        onClick={onLockNow}
        className="inline-flex items-center gap-2 px-3 py-1.5 text-sm rounded border border-gray-300 dark:border-[#5f6368] hover:bg-black/5 dark:hover:bg-white/10"
      >
        <Unlock size={14} aria-hidden /> Lock now
      </button>
      <form
        onSubmit={disable}
        className="grid gap-2 sm:grid-cols-[1fr_auto] pt-2 border-t border-gray-200 dark:border-[#5f6368]"
      >
        <input
          type="password"
          value={currentPin}
          onChange={(e) => setCurrentPin(e.target.value)}
          placeholder="Current PIN"
          aria-label="Current App Lock PIN to disable"
          autoComplete="current-password"
          className="px-3 py-1.5 text-sm rounded border border-gray-300 dark:border-[#5f6368] bg-transparent"
          disabled={busy}
        />
        <button
          type="submit"
          disabled={busy || currentPin.length === 0}
          className="px-3 py-1.5 text-sm rounded border border-[#d93025] text-[#d93025] hover:bg-[#d93025]/10 disabled:opacity-50"
        >
          {busy ? "Verifying…" : "Disable"}
        </button>
      </form>
    </div>
  );
}
