import { useEffect, useState } from "react";
import { X, Download, Upload, Folder } from "lucide-react";
import { open, save } from "@tauri-apps/plugin-dialog";
import { useStore } from "../store";
import { api } from "../api";

export function SettingsModal() {
  const { settingsOpen, closeSettings, dark, toggleDark, load, showToast } =
    useStore();
  const [dataDir, setDataDir] = useState<string>("");
  const [busy, setBusy] = useState(false);

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

  const importZip = async () => {
    try {
      const picked = await open({
        title: "Restore from Keepr backup",
        multiple: false,
        filters: [{ name: "Keepr backup", extensions: ["zip"] }],
      });
      if (!picked) return;
      if (
        !confirm(
          "Restoring will REPLACE all current notes with the contents of the backup. Continue?",
        )
      ) {
        return;
      }
      setBusy(true);
      await api.importZip(picked as string);
      await load();
      showToast("Backup restored");
    } catch (e: unknown) {
      showToast("Restore failed: " + String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div
      className="fixed inset-0 z-50 modal-backdrop grid place-items-center p-4"
      onClick={closeSettings}
    >
      <div
        className="w-full max-w-lg rounded-lg shadow-keep-hover bg-white dark:bg-[#2d2e30] text-gray-800 dark:text-gray-100"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between px-5 py-4 border-b border-gray-200 dark:border-[#5f6368]">
          <h2 className="text-lg font-medium">Settings</h2>
          <button
            onClick={closeSettings}
            className="p-2 rounded-full hover:bg-black/5 dark:hover:bg-white/10"
          >
            <X size={18} />
          </button>
        </div>

        <div className="px-5 py-4 space-y-5">
          <Row
            title="Theme"
            subtitle={dark ? "Dark" : "Light"}
            action={
              <button
                onClick={toggleDark}
                className="px-3 py-1.5 text-sm rounded border border-gray-300 dark:border-[#5f6368] hover:bg-black/5 dark:hover:bg-white/10"
              >
                Switch to {dark ? "light" : "dark"}
              </button>
            }
          />

          <Row
            title="Data folder"
            subtitle={dataDir || "—"}
            action={
              <span className="text-xs text-gray-500 dark:text-gray-400 flex items-center gap-1">
                <Folder size={14} /> local
              </span>
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
                <Download size={16} /> Export backup…
              </button>
              <button
                disabled={busy}
                onClick={importZip}
                className="flex items-center gap-2 px-3 py-2 text-sm rounded border border-gray-300 dark:border-[#5f6368] hover:bg-black/5 dark:hover:bg-white/10 disabled:opacity-50"
              >
                <Upload size={16} /> Restore backup…
              </button>
            </div>
          </div>
        </div>

        <div className="px-5 py-3 border-t border-gray-200 dark:border-[#5f6368] text-xs text-gray-500 dark:text-gray-400">
          Keepr v0.1.0 — offline-first Google Keep clone. MIT-licensed.
        </div>
      </div>
    </div>
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
