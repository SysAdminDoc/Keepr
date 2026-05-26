import { useState } from "react";
import { Shield, ShieldCheck, ShieldOff } from "lucide-react";
import { useStore } from "../store";
import { api } from "../api";

/**
 * NF-V0.5-C Settings → Private Vault. Three states:
 *   1. Not initialized → setup form (new password + confirm).
 *   2. Initialized + locked → unlock form (single password input).
 *   3. Initialized + unlocked → change-password form + Lock-vault-now.
 *
 * Argon2id KDF is the slow step (~150-300 ms per derive). Each button
 * shows a busy state during the call.
 */
export function VaultSection() {
  const vaultInitialized = useStore((s) => s.vaultInitialized);
  const vaultUnlocked = useStore((s) => s.vaultUnlocked);

  return (
    <div>
      <div className="font-medium flex items-center gap-2">
        {vaultUnlocked ? (
          <ShieldCheck size={16} className="text-[#34a853]" aria-hidden />
        ) : vaultInitialized ? (
          <Shield size={16} aria-hidden />
        ) : (
          <ShieldOff size={16} className="opacity-60" aria-hidden />
        )}{" "}
        Private Vault
      </div>
      <p className="text-sm text-gray-600 dark:text-gray-400 mt-1">
        Encrypts the title + body + checklist of any note you move into
        the vault with XChaCha20-Poly1305. The vault password derives a
        key-encryption-key (Argon2id) that wraps the data key — changing
        the password only re-wraps; nothing is re-encrypted. The vault
        password is <strong>not</strong> recoverable; if you forget it,
        the vaulted notes are unreadable. App Lock is a separate gate
        with its own PIN.
      </p>
      {!vaultInitialized ? (
        <SetupPanel />
      ) : !vaultUnlocked ? (
        <UnlockPanel />
      ) : (
        <UnlockedPanel />
      )}
    </div>
  );
}

function SetupPanel() {
  const showToast = useStore((s) => s.showToast);
  const refresh = useStore((s) => s.refreshVaultState);
  const [pw, setPw] = useState("");
  const [confirm, setConfirm] = useState("");
  const [busy, setBusy] = useState(false);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (busy) return;
    if (pw.length < 6) {
      showToast("Vault password must be at least 6 characters");
      return;
    }
    if (pw !== confirm) {
      showToast("Passwords don't match");
      return;
    }
    setBusy(true);
    try {
      await api.initVault(pw);
      await refresh();
      setPw("");
      setConfirm("");
      showToast("Vault created — unlocked");
    } catch (err) {
      showToast("Could not create vault: " + String(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <form onSubmit={submit} className="mt-3 grid gap-2 sm:grid-cols-2">
      <input
        type="password"
        value={pw}
        onChange={(e) => setPw(e.target.value)}
        placeholder="New vault password (min 6 chars)"
        aria-label="New vault password"
        autoComplete="new-password"
        className="px-3 py-1.5 text-sm rounded border border-gray-300 dark:border-[#5f6368] bg-transparent"
        disabled={busy}
      />
      <input
        type="password"
        value={confirm}
        onChange={(e) => setConfirm(e.target.value)}
        placeholder="Confirm password"
        aria-label="Confirm vault password"
        autoComplete="new-password"
        className="px-3 py-1.5 text-sm rounded border border-gray-300 dark:border-[#5f6368] bg-transparent"
        disabled={busy}
      />
      <button
        type="submit"
        disabled={busy}
        className="sm:col-span-2 px-3 py-2 text-sm rounded bg-[#1a73e8] text-white font-medium hover:bg-[#1557b0] disabled:opacity-50"
      >
        {busy ? "Creating…" : "Create vault"}
      </button>
    </form>
  );
}

function UnlockPanel() {
  const showToast = useStore((s) => s.showToast);
  const refresh = useStore((s) => s.refreshVaultState);
  const [pw, setPw] = useState("");
  const [busy, setBusy] = useState(false);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (busy || pw.length === 0) return;
    setBusy(true);
    try {
      const ok = await api.unlockVault(pw);
      if (ok) {
        await refresh();
        setPw("");
        showToast("Vault unlocked");
      } else {
        showToast("Incorrect vault password");
      }
    } catch (err) {
      showToast(String(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <form onSubmit={submit} className="mt-3 grid gap-2 sm:grid-cols-[1fr_auto]">
      <input
        type="password"
        value={pw}
        onChange={(e) => setPw(e.target.value)}
        placeholder="Vault password"
        aria-label="Vault password to unlock"
        autoComplete="current-password"
        className="px-3 py-1.5 text-sm rounded border border-gray-300 dark:border-[#5f6368] bg-transparent"
        disabled={busy}
      />
      <button
        type="submit"
        disabled={busy || pw.length === 0}
        className="px-3 py-1.5 text-sm rounded bg-[#1a73e8] text-white font-medium hover:bg-[#1557b0] disabled:opacity-50"
      >
        {busy ? "Verifying…" : "Unlock"}
      </button>
    </form>
  );
}

function UnlockedPanel() {
  const showToast = useStore((s) => s.showToast);
  const refresh = useStore((s) => s.refreshVaultState);
  const [current, setCurrent] = useState("");
  const [next, setNext] = useState("");
  const [busy, setBusy] = useState(false);

  const lockNow = async () => {
    try {
      await api.lockVault();
      await refresh();
      showToast("Vault locked");
    } catch (err) {
      showToast(String(err));
    }
  };

  const change = async (e: React.FormEvent) => {
    e.preventDefault();
    if (busy) return;
    if (next.length < 6) {
      showToast("New password must be at least 6 characters");
      return;
    }
    setBusy(true);
    try {
      await api.changeVaultPassword(current, next);
      setCurrent("");
      setNext("");
      showToast("Vault password changed");
    } catch (err) {
      showToast(String(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="mt-3 space-y-3">
      <div className="text-sm text-[#34a853] flex items-center gap-2">
        <ShieldCheck size={14} aria-hidden /> Vault is unlocked.
      </div>
      <button
        type="button"
        onClick={lockNow}
        className="inline-flex items-center gap-2 px-3 py-1.5 text-sm rounded border border-gray-300 dark:border-[#5f6368] hover:bg-black/5 dark:hover:bg-white/10"
      >
        <Shield size={14} aria-hidden /> Lock vault now
      </button>
      <form
        onSubmit={change}
        className="grid gap-2 sm:grid-cols-2 pt-2 border-t border-gray-200 dark:border-[#5f6368]"
      >
        <input
          type="password"
          value={current}
          onChange={(e) => setCurrent(e.target.value)}
          placeholder="Current password"
          aria-label="Current vault password"
          autoComplete="current-password"
          className="px-3 py-1.5 text-sm rounded border border-gray-300 dark:border-[#5f6368] bg-transparent"
          disabled={busy}
        />
        <input
          type="password"
          value={next}
          onChange={(e) => setNext(e.target.value)}
          placeholder="New password (min 6 chars)"
          aria-label="New vault password"
          autoComplete="new-password"
          className="px-3 py-1.5 text-sm rounded border border-gray-300 dark:border-[#5f6368] bg-transparent"
          disabled={busy}
        />
        <button
          type="submit"
          disabled={busy || current.length === 0 || next.length === 0}
          className="sm:col-span-2 px-3 py-2 text-sm rounded border border-gray-300 dark:border-[#5f6368] hover:bg-black/5 dark:hover:bg-white/10 disabled:opacity-50"
        >
          {busy ? "Rewrapping…" : "Change vault password"}
        </button>
      </form>
    </div>
  );
}
