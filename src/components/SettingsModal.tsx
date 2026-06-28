import { useEffect, useRef, useState } from "react";
import { X, Download, Upload, FolderOpen } from "lucide-react";
import { open, save } from "@tauri-apps/plugin-dialog";
import {
  ACCENT_PRESETS,
  MAX_NOTE_FONT_SIZE,
  MIN_NOTE_FONT_SIZE,
  useStore,
} from "../store";
import { api } from "../api";
import { useEscape } from "../hooks/useEscape";
import { useFocusTrap } from "../hooks/useFocusTrap";
import { ConfirmDialog } from "./ConfirmDialog";
import { AppLockSection } from "./AppLockSection";
import { VaultSection } from "./VaultSection";
import { VoiceTranscriptionSection } from "./VoiceTranscriptionSection";
import { WebClipperSection } from "./WebClipperSection";

export function SettingsModal() {
  const settingsOpen = useStore((s) => s.settingsOpen);
  const closeSettings = useStore((s) => s.closeSettings);
  const themeMode = useStore((s) => s.themeMode);
  const setThemeMode = useStore((s) => s.setThemeMode);
  const trashRetentionDays = useStore((s) => s.trashRetentionDays);
  const setTrashRetentionDays = useStore((s) => s.setTrashRetentionDays);
  const moveCheckedToBottom = useStore((s) => s.moveCheckedToBottom);
  const setMoveCheckedToBottom = useStore((s) => s.setMoveCheckedToBottom);
  const autoBackupCadence = useStore((s) => s.autoBackupCadence);
  const setAutoBackupCadence = useStore((s) => s.setAutoBackupCadence);
  const autoBackupFolder = useStore((s) => s.autoBackupFolder);
  const autoBackupRetention = useStore((s) => s.autoBackupRetention);
  const setAutoBackupRetention = useStore((s) => s.setAutoBackupRetention);
  const setAutoBackupFolder = useStore((s) => s.setAutoBackupFolder);
  const autoBackupLastAt = useStore((s) => s.autoBackupLastAt);
  const accentColor = useStore((s) => s.accentColor);
  const setAccentColor = useStore((s) => s.setAccentColor);
  const noteFontSize = useStore((s) => s.noteFontSize);
  const setNoteFontSize = useStore((s) => s.setNoteFontSize);
  const resetNoteFontSize = useStore((s) => s.resetNoteFontSize);
  const load = useStore((s) => s.load);
  const showToast = useStore((s) => s.showToast);

  const [dataDir, setDataDir] = useState<string>("");
  const [logDir, setLogDir] = useState<string>("");
  const [appVersion, setAppVersion] = useState<string>("");
  const [busy, setBusy] = useState(false);
  const [pendingRestoreSrc, setPendingRestoreSrc] = useState<string | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  useEscape(settingsOpen, closeSettings);
  useFocusTrap(containerRef, settingsOpen);

  useEffect(() => {
    if (!settingsOpen) return;
    api.getAppVersion().then(setAppVersion).catch(() => setAppVersion(""));
    api.getDataDir().then(setDataDir).catch(() => {});
    api.getLogDir().then(setLogDir).catch(() => {});
  }, [settingsOpen]);

  if (!settingsOpen) return null;

  const exportZip = async () => {
    try {
      const stamp = new Date().toISOString().replace(/[:.]/g, "-").slice(0, 19);
      const filename = `keepr-backup-${stamp}.zip`;
      const dest = await save({
        title: "Save Keepr backup",
        defaultPath: filename,
        filters: [{ name: "Keepr backup", extensions: ["zip"] }],
      });
      if (!dest) return;
      setBusy(true);
      const written = await api.exportZip(dest as string);
      showToast(`Backup saved to ${written}`);
    } catch (e: unknown) {
      showToast("Backup failed: " + String(e));
    } finally {
      setBusy(false);
    }
  };

  const pickAndStageRestore = async () => {
    try {
      const picked = await open({
        title: "Restore from Keepr backup",
        multiple: false,
        filters: [{ name: "Keepr backup", extensions: ["zip"] }],
      });
      if (!picked) return;
      setPendingRestoreSrc(picked as string);
    } catch (e: unknown) {
      showToast("Could not open file: " + String(e));
    }
  };

  const performRestore = async () => {
    if (!pendingRestoreSrc) return;
    const src = pendingRestoreSrc;
    setPendingRestoreSrc(null);
    setBusy(true);
    try {
      await api.importZip(src);
      await load();
      showToast("Backup restored");
    } catch (e: unknown) {
      showToast("Restore failed: " + String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      <div
        className="fixed inset-0 z-50 modal-backdrop grid place-items-center p-4"
        onClick={closeSettings}
        role="dialog"
        aria-modal="true"
        aria-labelledby="settings-title"
      >
        <div
          ref={containerRef}
          className="w-full max-w-2xl max-h-[90vh] flex flex-col rounded-lg shadow-keep-hover bg-white dark:bg-[#2d2e30] text-gray-800 dark:text-gray-100"
          onClick={(e) => e.stopPropagation()}
        >
          <div className="flex items-center justify-between px-5 py-4 border-b border-gray-200 dark:border-[#5f6368] shrink-0">
            <h2 id="settings-title" className="text-lg font-medium">
              Settings
            </h2>
            <button
              onClick={closeSettings}
              aria-label="Close settings"
              title="Close settings"
              className="p-2 rounded-full hover:bg-black/5 dark:hover:bg-white/10"
            >
              <X size={18} />
            </button>
          </div>

          <div className="px-5 py-4 space-y-5 overflow-y-auto">
            <Row
              title="Theme"
              subtitle={
                themeMode === "system"
                  ? "Follows your operating system"
                  : themeMode === "dark"
                  ? "Dark"
                  : "Light"
              }
              action={
                <div
                  role="radiogroup"
                  aria-label="Theme"
                  className="inline-flex rounded border border-gray-300 dark:border-[#5f6368] overflow-hidden text-sm"
                >
                  {(["light", "dark", "system"] as const).map((mode) => (
                    <button
                      key={mode}
                      type="button"
                      role="radio"
                      aria-checked={themeMode === mode}
                      onClick={() => setThemeMode(mode)}
                      className={
                        themeMode === mode
                          ? "px-3 py-1.5 bg-[var(--keepr-accent)] text-white"
                          : "px-3 py-1.5 hover:bg-black/5 dark:hover:bg-white/10"
                      }
                    >
                      {mode === "light"
                        ? "Light"
                        : mode === "dark"
                        ? "Dark"
                        : "System"}
                    </button>
                  ))}
                </div>
              }
            />

            <Row
              title="Accent color"
              subtitle={
                ACCENT_PRESETS.find((p) => p.color === accentColor)?.name
                  ?? "Custom"
              }
              action={
                <div
                  role="radiogroup"
                  aria-label="Accent color"
                  className="flex items-center gap-2"
                >
                  {ACCENT_PRESETS.map((p) => {
                    const selected = p.color === accentColor;
                    return (
                      <button
                        key={p.color}
                        type="button"
                        role="radio"
                        aria-checked={selected}
                        aria-label={p.name}
                        title={p.name}
                        onClick={() => setAccentColor(p.color)}
                        className={
                          selected
                            ? "w-7 h-7 rounded border-2 border-gray-800 dark:border-white"
                            : "w-7 h-7 rounded border border-black/10 dark:border-white/10 hover:scale-110 transition-transform"
                        }
                        style={{ backgroundColor: p.color }}
                      />
                    );
                  })}
                </div>
              }
            />

            <Row
              title="Note text size"
              subtitle={`${noteFontSize}px — applies to note bodies and checklist items`}
              action={
                <div className="flex items-center gap-3">
                  <input
                    type="range"
                    aria-label="Note text size"
                    min={MIN_NOTE_FONT_SIZE}
                    max={MAX_NOTE_FONT_SIZE}
                    step={1}
                    value={noteFontSize}
                    onChange={(e) => setNoteFontSize(parseInt(e.target.value, 10))}
                    className="w-32 accent-[var(--keepr-accent)]"
                  />
                  <span className="text-xs text-gray-500 dark:text-gray-400 w-10 text-right tabular-nums">
                    {noteFontSize}px
                  </span>
                  <button
                    type="button"
                    onClick={resetNoteFontSize}
                    className="text-xs px-2 py-1 rounded border border-gray-300 dark:border-[#5f6368] hover:bg-black/5 dark:hover:bg-white/10"
                  >
                    Reset
                  </button>
                </div>
              }
            />

            <Row
              title="Data folder"
              subtitle={dataDir || "—"}
              action={
                <div className="flex items-center gap-2">
                  <button
                    type="button"
                    onClick={async () => {
                      try {
                        await api.openAppDir("data");
                      } catch (e) {
                        showToast("Could not open: " + String(e));
                      }
                    }}
                    disabled={!dataDir}
                    className="text-xs px-2 py-1 rounded border border-gray-300 dark:border-[#5f6368] hover:bg-black/5 dark:hover:bg-white/10 disabled:opacity-50 flex items-center gap-1"
                  >
                    <FolderOpen size={12} aria-hidden /> Open
                  </button>
                  <button
                    type="button"
                    onClick={async () => {
                      if (!dataDir) return;
                      try {
                        await navigator.clipboard.writeText(dataDir);
                        showToast("Data folder path copied");
                      } catch {
                        showToast("Copy failed — path: " + dataDir);
                      }
                    }}
                    disabled={!dataDir}
                    className="text-xs px-2 py-1 rounded border border-gray-300 dark:border-[#5f6368] hover:bg-black/5 dark:hover:bg-white/10 disabled:opacity-50"
                  >
                    Copy path
                  </button>
                </div>
              }
            />

            <Row
              title="Log folder"
              subtitle={logDir || "—"}
              action={
                <div className="flex items-center gap-2">
                  <button
                    type="button"
                    onClick={async () => {
                      try {
                        await api.openAppDir("log");
                      } catch (e) {
                        showToast("Could not open: " + String(e));
                      }
                    }}
                    disabled={!logDir}
                    className="text-xs px-2 py-1 rounded border border-gray-300 dark:border-[#5f6368] hover:bg-black/5 dark:hover:bg-white/10 disabled:opacity-50 flex items-center gap-1"
                  >
                    <FolderOpen size={12} aria-hidden /> Open
                  </button>
                  <button
                    type="button"
                    onClick={async () => {
                      if (!logDir) return;
                      try {
                        await navigator.clipboard.writeText(logDir);
                        showToast("Log folder path copied");
                      } catch {
                        showToast("Copy failed — path: " + logDir);
                      }
                    }}
                    disabled={!logDir}
                    className="text-xs px-2 py-1 rounded border border-gray-300 dark:border-[#5f6368] hover:bg-black/5 dark:hover:bg-white/10 disabled:opacity-50"
                  >
                    Copy path
                  </button>
                </div>
              }
            />

            <Row
              title="Move checked items to bottom"
              subtitle={
                moveCheckedToBottom
                  ? "Ticked items collapse into a group below the list"
                  : "Items stay in the order you added them"
              }
              action={
                <input
                  type="checkbox"
                  checked={moveCheckedToBottom}
                  onChange={(e) => setMoveCheckedToBottom(e.target.checked)}
                  aria-label="Move checked items to bottom"
                  className="w-5 h-5 accent-[var(--keepr-accent)]"
                />
              }
            />

            <Row
              title="Auto-empty Trash"
              subtitle={
                trashRetentionDays === 0
                  ? "Never — keep trashed notes forever"
                  : trashRetentionDays === 1
                  ? "After 1 day"
                  : `After ${trashRetentionDays} days`
              }
              action={
                <input
                  type="number"
                  min={0}
                  max={3650}
                  value={trashRetentionDays}
                  onChange={(e) => {
                    const v = parseInt(e.target.value, 10);
                    if (!Number.isNaN(v)) setTrashRetentionDays(v);
                  }}
                  aria-label="Auto-empty Trash after how many days (0 = never)"
                  className="w-20 px-2 py-1 text-sm rounded border border-gray-300 dark:border-[#5f6368] bg-transparent text-right"
                />
              }
            />

            <div>
              <div className="font-medium">Auto-backup</div>
              <p className="text-sm text-gray-600 dark:text-gray-400 mt-1">
                Keepr writes a timestamped <code>.zip</code> into the folder
                you pick on the cadence you choose. Point the folder at your
                Google Drive / OneDrive / Dropbox sync folder for a cloud
                copy with no plumbing. Backups run while Keepr is open;
                missed windows catch up on the next launch.
              </p>
              <div className="flex items-center gap-2 mt-3 flex-wrap">
                <select
                  aria-label="Auto-backup cadence"
                  value={autoBackupCadence}
                  onChange={(e) =>
                    setAutoBackupCadence(
                      e.target.value as typeof autoBackupCadence,
                    )
                  }
                  className="px-3 py-1.5 text-sm rounded border border-gray-300 dark:border-[#5f6368] bg-transparent"
                >
                  <option value="off">Off</option>
                  <option value="daily">Daily</option>
                  <option value="weekly">Weekly</option>
                </select>
                <button
                  type="button"
                  onClick={async () => {
                    const picked = await open({
                      title: "Pick auto-backup folder",
                      directory: true,
                      multiple: false,
                    });
                    if (picked) setAutoBackupFolder(picked as string);
                  }}
                  className="flex items-center gap-2 px-3 py-1.5 text-sm rounded border border-gray-300 dark:border-[#5f6368] hover:bg-black/5 dark:hover:bg-white/10"
                >
                  <FolderOpen size={14} aria-hidden />
                  {autoBackupFolder ? "Change folder…" : "Pick folder…"}
                </button>
                {autoBackupFolder && (
                  <button
                    type="button"
                    onClick={() => setAutoBackupFolder(null)}
                    className="text-xs text-gray-500 dark:text-gray-400 underline"
                  >
                    Forget folder
                  </button>
                )}
              </div>
              {autoBackupFolder && (
                <div className="mt-2 text-xs text-gray-500 dark:text-gray-400 truncate">
                  Folder: <code>{autoBackupFolder}</code>
                </div>
              )}
              {autoBackupLastAt && (
                <div className="mt-1 text-xs text-gray-500 dark:text-gray-400">
                  Last backup: {new Date(autoBackupLastAt).toLocaleString()}
                </div>
              )}
              {autoBackupCadence !== "off" && (
                <div className="mt-3 flex items-center gap-2 text-xs">
                  <label htmlFor="autobackup-retention" className="text-gray-600 dark:text-gray-400">
                    Keep latest
                  </label>
                  <input
                    id="autobackup-retention"
                    type="number"
                    min={0}
                    max={365}
                    value={autoBackupRetention}
                    onChange={(e) => setAutoBackupRetention(parseInt(e.target.value, 10) || 0)}
                    className="w-16 text-right px-1 py-0.5 rounded border border-gray-300 dark:border-[#5f6368] bg-transparent"
                  />
                  <span className="text-gray-600 dark:text-gray-400">
                    auto-backups in this folder (0 = unlimited; older ones are deleted)
                  </span>
                </div>
              )}
            </div>

            <AppLockSection />

            <VaultSection />

            <VoiceTranscriptionSection />

            <WebClipperSection />

            <div>
              <div className="font-medium">Backup / Restore</div>
              <p className="text-sm text-gray-600 dark:text-gray-400 mt-1">
                Export your full note database to a single .zip file. Drop it into
                your Google Drive desktop folder for a cloud copy. Restore on any
                machine to bring everything back.
              </p>
              <div className="flex gap-2 mt-3">
                <button
                  disabled={busy}
                  onClick={exportZip}
                  className="flex items-center gap-2 px-3 py-2 text-sm rounded border border-gray-300 dark:border-[#5f6368] hover:bg-black/5 dark:hover:bg-white/10 disabled:opacity-50"
                >
                  <Download size={16} aria-hidden /> Export backup…
                </button>
                <button
                  disabled={busy}
                  onClick={pickAndStageRestore}
                  className="flex items-center gap-2 px-3 py-2 text-sm rounded border border-gray-300 dark:border-[#5f6368] hover:bg-black/5 dark:hover:bg-white/10 disabled:opacity-50"
                >
                  <Upload size={16} aria-hidden /> Restore backup…
                </button>
              </div>
            </div>

            <div>
              <div className="font-medium">Markdown vault export &amp; Takeout import</div>
              <p className="text-sm text-gray-600 dark:text-gray-400 mt-1">
                Export every note as a separate <code>.md</code> file with
                YAML frontmatter, ready to open in Obsidian, Joplin, or any
                text editor. Or drop your Google Takeout ZIP to migrate
                straight from Keep — labels, colors, lists, and images
                are preserved.
              </p>
              <div className="flex gap-2 mt-3 flex-wrap">
                <button
                  disabled={busy}
                  onClick={async () => {
                    try {
                      const picked = await open({
                        title: "Pick vault destination folder",
                        directory: true,
                        multiple: false,
                      });
                      if (!picked) return;
                      setBusy(true);
                      const writtenTo = await api.exportVault(picked as string);
                      showToast(`Vault exported to ${writtenTo}`);
                    } catch (e) {
                      showToast("Vault export failed: " + String(e));
                    } finally {
                      setBusy(false);
                    }
                  }}
                  className="flex items-center gap-2 px-3 py-2 text-sm rounded border border-gray-300 dark:border-[#5f6368] hover:bg-black/5 dark:hover:bg-white/10 disabled:opacity-50"
                >
                  <Download size={16} aria-hidden /> Export as Markdown vault…
                </button>
                <button
                  disabled={busy}
                  onClick={async () => {
                    try {
                      const picked = await open({
                        title: "Import Google Takeout",
                        multiple: false,
                        filters: [{ name: "Takeout ZIP", extensions: ["zip"] }],
                      });
                      if (!picked) return;
                      setBusy(true);
                      const count = await api.importTakeout(picked as string);
                      await load();
                      showToast(`Imported ${count} notes from Takeout`);
                    } catch (e) {
                      showToast("Takeout import failed: " + String(e));
                    } finally {
                      setBusy(false);
                    }
                  }}
                  className="flex items-center gap-2 px-3 py-2 text-sm rounded border border-gray-300 dark:border-[#5f6368] hover:bg-black/5 dark:hover:bg-white/10 disabled:opacity-50"
                >
                  <Upload size={16} aria-hidden /> Import Google Takeout…
                </button>
                <button
                  disabled={busy}
                  onClick={async () => {
                    try {
                      const stamp = new Date()
                        .toISOString()
                        .replace(/[:.]/g, "-")
                        .slice(0, 19);
                      const dest = await save({
                        title: "Save reminders as iCalendar",
                        defaultPath: `keepr-reminders-${stamp}.ics`,
                        filters: [{ name: "iCalendar", extensions: ["ics"] }],
                      });
                      if (!dest) return;
                      setBusy(true);
                      const summary = await api.exportRemindersIcs(dest as string);
                      showToast(summary);
                    } catch (e) {
                      showToast("ICS export failed: " + String(e));
                    } finally {
                      setBusy(false);
                    }
                  }}
                  className="flex items-center gap-2 px-3 py-2 text-sm rounded border border-gray-300 dark:border-[#5f6368] hover:bg-black/5 dark:hover:bg-white/10 disabled:opacity-50"
                >
                  <Download size={16} aria-hidden /> Export reminders as
                  iCalendar (.ics)…
                </button>
              </div>
            </div>
          </div>

          <div className="px-5 py-3 border-t border-gray-200 dark:border-[#5f6368] text-xs text-gray-500 dark:text-gray-400">
            Keepr{appVersion ? ` v${appVersion}` : ""} — offline-first Google Keep clone. MIT-licensed.
          </div>
        </div>
      </div>

      <ConfirmDialog
        open={pendingRestoreSrc !== null}
        title="Restore from backup?"
        body="Restoring will REPLACE all current notes with the contents of the selected backup. Your existing database is snapshotted to keepr.db.prev and can be recovered manually."
        confirmLabel="Restore"
        cancelLabel="Cancel"
        destructive
        onConfirm={performRestore}
        onCancel={() => setPendingRestoreSrc(null)}
      />
    </>
  );
}

function Row({
  title,
  subtitle,
  action,
}: {
  title: string;
  subtitle: string;
  action: React.ReactNode;
}) {
  return (
    <div className="flex items-center justify-between gap-4">
      <div className="min-w-0">
        <div className="font-medium">{title}</div>
        <div className="text-sm text-gray-600 dark:text-gray-400 truncate">
          {subtitle}
        </div>
      </div>
      <div className="shrink-0">{action}</div>
    </div>
  );
}
