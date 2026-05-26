import { useEffect, useRef, useState } from "react";
import { X, Download, Upload, Folder } from "lucide-react";
import { open, save } from "@tauri-apps/plugin-dialog";
import { useStore } from "../store";
import { api } from "../api";
import { useEscape } from "../hooks/useEscape";
import { useFocusTrap } from "../hooks/useFocusTrap";
import { ConfirmDialog } from "./ConfirmDialog";

export function SettingsModal() {
  const settingsOpen = useStore((s) => s.settingsOpen);
  const closeSettings = useStore((s) => s.closeSettings);
  const themeMode = useStore((s) => s.themeMode);
  const setThemeMode = useStore((s) => s.setThemeMode);
  const trashRetentionDays = useStore((s) => s.trashRetentionDays);
  const setTrashRetentionDays = useStore((s) => s.setTrashRetentionDays);
  const moveCheckedToBottom = useStore((s) => s.moveCheckedToBottom);
  const setMoveCheckedToBottom = useStore((s) => s.setMoveCheckedToBottom);
  const load = useStore((s) => s.load);
  const showToast = useStore((s) => s.showToast);

  const [dataDir, setDataDir] = useState<string>("");
  const [busy, setBusy] = useState(false);
  const [pendingRestoreSrc, setPendingRestoreSrc] = useState<string | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  useEscape(settingsOpen, closeSettings);
  useFocusTrap(containerRef, settingsOpen);

  useEffect(() => {
    if (!settingsOpen) return;
    api.getDataDir().then(setDataDir).catch(() => {});
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
          className="w-full max-w-lg rounded-lg shadow-keep-hover bg-white dark:bg-[#2d2e30] text-gray-800 dark:text-gray-100"
          onClick={(e) => e.stopPropagation()}
        >
          <div className="flex items-center justify-between px-5 py-4 border-b border-gray-200 dark:border-[#5f6368]">
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

          <div className="px-5 py-4 space-y-5">
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
                          ? "px-3 py-1.5 bg-[#1a73e8] text-white"
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
              title="Data folder"
              subtitle={dataDir || "—"}
              action={
                <span className="text-xs text-gray-500 dark:text-gray-400 flex items-center gap-1">
                  <Folder size={14} aria-hidden /> local
                </span>
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
                  className="w-5 h-5 accent-[#1a73e8]"
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
          </div>

          <div className="px-5 py-3 border-t border-gray-200 dark:border-[#5f6368] text-xs text-gray-500 dark:text-gray-400">
            Keepr v0.2.0 — offline-first Google Keep clone. MIT-licensed.
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
