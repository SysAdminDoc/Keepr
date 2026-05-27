import { useEffect, useState } from "react";
import { Shield, ShieldCheck, ShieldOff, Key, X, Copy } from "lucide-react";
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
        className="sm:col-span-2 px-3 py-2 text-sm rounded bg-[var(--keepr-accent)] text-white font-medium hover:bg-[var(--keepr-accent-hover)] disabled:opacity-50"
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
  const [hasSeed, setHasSeed] = useState(false);
  const [recoverOpen, setRecoverOpen] = useState(false);

  useEffect(() => {
    api.vaultHasRecoverySeed().then(setHasSeed).catch(() => {});
  }, []);

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
    <>
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
          className="px-3 py-1.5 text-sm rounded bg-[var(--keepr-accent)] text-white font-medium hover:bg-[var(--keepr-accent-hover)] disabled:opacity-50"
        >
          {busy ? "Verifying…" : "Unlock"}
        </button>
      </form>
      {hasSeed && (
        <button
          type="button"
          onClick={() => setRecoverOpen(true)}
          className="mt-2 text-xs text-[var(--keepr-accent)] hover:underline"
        >
          Forgot password? Recover with seed phrase…
        </button>
      )}
      {recoverOpen && (
        <RecoverWithSeedModal
          onClose={() => setRecoverOpen(false)}
          onRecovered={async () => {
            setRecoverOpen(false);
            await refresh();
            showToast("Vault recovered + unlocked");
          }}
        />
      )}
    </>
  );
}

function RecoverWithSeedModal({
  onClose,
  onRecovered,
}: {
  onClose: () => void;
  onRecovered: () => void | Promise<void>;
}) {
  const showToast = useStore((s) => s.showToast);
  const [phrase, setPhrase] = useState("");
  const [newPw, setNewPw] = useState("");
  const [confirm, setConfirm] = useState("");
  const [busy, setBusy] = useState(false);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (busy) return;
    const words = phrase.trim().split(/\s+/);
    if (words.length !== 12) {
      showToast("Recovery phrase must be exactly 12 words");
      return;
    }
    if (newPw.length < 6) {
      showToast("New password must be at least 6 characters");
      return;
    }
    if (newPw !== confirm) {
      showToast("Passwords don't match");
      return;
    }
    setBusy(true);
    try {
      await api.recoverVaultWithSeed(words.join(" "), newPw);
      await onRecovered();
    } catch (err) {
      showToast("Recovery failed: " + String(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div
      className="fixed inset-0 z-[55] modal-backdrop grid place-items-center p-4"
      role="dialog"
      aria-modal="true"
      aria-label="Recover vault with seed phrase"
      onClick={onClose}
    >
      <div
        className="w-full max-w-md rounded-lg shadow-2xl border border-gray-300 dark:border-[#5f6368] bg-white dark:bg-[#2d2e30] p-5"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between mb-3">
          <h2 className="text-base font-medium flex items-center gap-2">
            <Key size={16} aria-hidden /> Recover with seed phrase
          </h2>
          <button
            type="button"
            onClick={onClose}
            aria-label="Cancel"
            className="p-1 rounded hover:bg-black/5 dark:hover:bg-white/10"
          >
            <X size={16} />
          </button>
        </div>
        <p className="text-sm text-gray-600 dark:text-gray-400 mb-3">
          Enter the 12-word recovery phrase you saved when setting up the
          seed, then a new vault password. The vault contents are
          unchanged — this only swaps out the password.
        </p>
        <form onSubmit={submit} className="space-y-2">
          <textarea
            value={phrase}
            onChange={(e) => setPhrase(e.target.value)}
            placeholder="word1 word2 word3 … word12"
            aria-label="12-word recovery phrase"
            rows={3}
            className="w-full px-3 py-2 text-sm rounded border border-gray-300 dark:border-[#5f6368] bg-transparent"
            disabled={busy}
          />
          <input
            type="password"
            value={newPw}
            onChange={(e) => setNewPw(e.target.value)}
            placeholder="New vault password (min 6 chars)"
            aria-label="New vault password"
            autoComplete="new-password"
            className="w-full px-3 py-1.5 text-sm rounded border border-gray-300 dark:border-[#5f6368] bg-transparent"
            disabled={busy}
          />
          <input
            type="password"
            value={confirm}
            onChange={(e) => setConfirm(e.target.value)}
            placeholder="Confirm new password"
            aria-label="Confirm new vault password"
            autoComplete="new-password"
            className="w-full px-3 py-1.5 text-sm rounded border border-gray-300 dark:border-[#5f6368] bg-transparent"
            disabled={busy}
          />
          <button
            type="submit"
            disabled={busy}
            className="w-full px-3 py-2 text-sm rounded text-white font-medium bg-[var(--keepr-accent)] hover:bg-[var(--keepr-accent-hover)] disabled:opacity-50"
          >
            {busy ? "Recovering…" : "Recover vault"}
          </button>
        </form>
      </div>
    </div>
  );
}

function UnlockedPanel() {
  const showToast = useStore((s) => s.showToast);
  const refresh = useStore((s) => s.refreshVaultState);
  const [current, setCurrent] = useState("");
  const [next, setNext] = useState("");
  const [busy, setBusy] = useState(false);
  const [hasSeed, setHasSeed] = useState(false);
  const [seedSetupOpen, setSeedSetupOpen] = useState(false);
  const [seedPhrase, setSeedPhrase] = useState<string | null>(null);

  useEffect(() => {
    api.vaultHasRecoverySeed().then(setHasSeed).catch(() => {});
  }, []);

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

      {/* v0.21.1 — opt-in recovery seed. If a seed exists we expose a
          Remove button; otherwise a Set-up button that prompts for the
          current password and shows the seed once. */}
      <div className="pt-2 border-t border-gray-200 dark:border-[#5f6368]">
        <div className="text-sm font-medium flex items-center gap-2">
          <Key size={14} aria-hidden /> Recovery seed
          <span className="text-xs text-gray-500 dark:text-gray-400 font-normal">
            {hasSeed ? "active" : "not set"}
          </span>
        </div>
        <p className="text-xs text-gray-600 dark:text-gray-400 mt-1">
          A 12-word phrase that can unlock this vault even if you forget
          the password. <strong>Opt-in</strong>: enabling this trades the
          "no recovery, no exceptions" guarantee for a recoverable seed.
          Write the phrase down somewhere offline — Keepr never stores
          it in plaintext after you close the dialog.
        </p>
        {hasSeed ? (
          <button
            type="button"
            onClick={async () => {
              if (!confirm("Remove the recovery seed? Without it, a forgotten password means the vault contents are unrecoverable.")) return;
              try {
                await api.removeVaultRecoverySeed();
                setHasSeed(false);
                showToast("Recovery seed removed");
              } catch (err) {
                showToast(String(err));
              }
            }}
            className="mt-2 text-xs text-red-600 dark:text-red-400 hover:underline"
          >
            Remove recovery seed…
          </button>
        ) : (
          <button
            type="button"
            onClick={() => setSeedSetupOpen(true)}
            className="mt-2 inline-flex items-center gap-2 px-3 py-1.5 text-sm rounded border border-gray-300 dark:border-[#5f6368] hover:bg-black/5 dark:hover:bg-white/10"
          >
            <Key size={14} aria-hidden /> Set up recovery seed…
          </button>
        )}
      </div>

      {seedSetupOpen && !seedPhrase && (
        <SetupSeedModal
          onClose={() => setSeedSetupOpen(false)}
          onGenerated={(phrase) => setSeedPhrase(phrase)}
        />
      )}
      {seedPhrase && (
        <ShowSeedModal
          phrase={seedPhrase}
          onAcknowledged={() => {
            setSeedPhrase(null);
            setSeedSetupOpen(false);
            setHasSeed(true);
          }}
        />
      )}
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

function SetupSeedModal({
  onClose,
  onGenerated,
}: {
  onClose: () => void;
  onGenerated: (phrase: string) => void;
}) {
  const showToast = useStore((s) => s.showToast);
  const [pw, setPw] = useState("");
  const [busy, setBusy] = useState(false);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (busy || pw.length === 0) return;
    setBusy(true);
    try {
      const phrase = await api.setupVaultRecoverySeed(pw);
      onGenerated(phrase);
    } catch (err) {
      showToast("Could not set up seed: " + String(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div
      className="fixed inset-0 z-[55] modal-backdrop grid place-items-center p-4"
      role="dialog"
      aria-modal="true"
      aria-label="Set up recovery seed"
      onClick={onClose}
    >
      <div
        className="w-full max-w-sm rounded-lg shadow-2xl border border-gray-300 dark:border-[#5f6368] bg-white dark:bg-[#2d2e30] p-5"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between mb-3">
          <h2 className="text-base font-medium">Set up recovery seed</h2>
          <button
            type="button"
            onClick={onClose}
            aria-label="Cancel"
            className="p-1 rounded hover:bg-black/5 dark:hover:bg-white/10"
          >
            <X size={16} />
          </button>
        </div>
        <p className="text-sm text-gray-600 dark:text-gray-400 mb-3">
          Confirm your current vault password. Keepr will then generate
          a 12-word phrase that can recover this vault — write it down
          somewhere offline before closing the next dialog.
        </p>
        <form onSubmit={submit} className="space-y-2">
          <input
            type="password"
            value={pw}
            onChange={(e) => setPw(e.target.value)}
            placeholder="Current vault password"
            aria-label="Current vault password"
            autoComplete="current-password"
            className="w-full px-3 py-1.5 text-sm rounded border border-gray-300 dark:border-[#5f6368] bg-transparent"
            disabled={busy}
          />
          <button
            type="submit"
            disabled={busy || pw.length === 0}
            className="w-full px-3 py-2 text-sm rounded text-white font-medium bg-[var(--keepr-accent)] hover:bg-[var(--keepr-accent-hover)] disabled:opacity-50"
          >
            {busy ? "Generating…" : "Generate recovery seed"}
          </button>
        </form>
      </div>
    </div>
  );
}

function ShowSeedModal({
  phrase,
  onAcknowledged,
}: {
  phrase: string;
  onAcknowledged: () => void;
}) {
  const showToast = useStore((s) => s.showToast);
  const [acknowledged, setAcknowledged] = useState(false);
  const words = phrase.split(/\s+/);

  return (
    <div
      className="fixed inset-0 z-[56] modal-backdrop grid place-items-center p-4"
      role="dialog"
      aria-modal="true"
      aria-label="Your recovery seed"
    >
      <div className="w-full max-w-lg rounded-lg shadow-2xl border border-gray-300 dark:border-[#5f6368] bg-white dark:bg-[#2d2e30] p-5">
        <h2 className="text-base font-medium mb-2 flex items-center gap-2">
          <Key size={16} aria-hidden /> Your recovery seed
        </h2>
        <p className="text-sm text-red-600 dark:text-red-400 mb-3">
          This phrase is shown once and never again. Write it down on
          paper or save it in a password manager <strong>offline</strong>.
          Anyone with these words can decrypt this vault.
        </p>
        <ol className="grid grid-cols-2 sm:grid-cols-3 gap-1.5 text-sm font-mono p-3 rounded border border-gray-300 dark:border-[#5f6368] bg-black/5 dark:bg-white/5">
          {words.map((w, i) => (
            <li key={i} className="flex items-center gap-2">
              <span className="text-xs opacity-60 w-5 text-right tabular-nums">{i + 1}.</span>
              {w}
            </li>
          ))}
        </ol>
        <div className="mt-3 flex items-center gap-2">
          <button
            type="button"
            onClick={async () => {
              try {
                await navigator.clipboard.writeText(phrase);
                showToast("Recovery seed copied — paste into your password manager");
              } catch {
                showToast("Copy failed — write the words down manually");
              }
            }}
            className="inline-flex items-center gap-2 px-3 py-1.5 text-xs rounded border border-gray-300 dark:border-[#5f6368] hover:bg-black/5 dark:hover:bg-white/10"
          >
            <Copy size={12} aria-hidden /> Copy to clipboard
          </button>
        </div>
        <label className="mt-4 flex items-start gap-2 text-sm">
          <input
            type="checkbox"
            checked={acknowledged}
            onChange={(e) => setAcknowledged(e.target.checked)}
            className="mt-1"
          />
          <span>I have written down (or otherwise saved) all 12 words. I understand that if I lose this phrase AND my password, the vault contents are unrecoverable.</span>
        </label>
        <button
          type="button"
          onClick={onAcknowledged}
          disabled={!acknowledged}
          className="mt-3 w-full px-3 py-2 text-sm rounded text-white font-medium bg-[var(--keepr-accent)] hover:bg-[var(--keepr-accent-hover)] disabled:opacity-50"
        >
          Done
        </button>
      </div>
    </div>
  );
}
