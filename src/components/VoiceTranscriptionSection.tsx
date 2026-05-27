import { useEffect, useRef, useState } from "react";
import { Download, Trash2, MicVocal, CheckCircle2, Loader2 } from "lucide-react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { api } from "../api";
import { useStore } from "../store";
import type { ModelDownloadProgress, SpeechModelStatus } from "../types";

/**
 * v0.23.0 — Settings → Voice transcription.
 *
 * Surfaces the whisper.cpp speech-to-text model lifecycle. The model
 * (~57 MB ggml-base.en-q5_1.bin) is NOT bundled. The user opts in here;
 * download streams once with a progress bar; after that, transcription
 * runs fully offline forever.
 *
 * Three states:
 *   - Not downloaded → show size + offline guarantee + "Download" button.
 *   - Downloading    → show progress bar + cancel-disabled (cancellation
 *                      mid-stream would leave a partial; we don't pretend
 *                      to support it for the v1).
 *   - Downloaded     → show "Ready" badge + on-disk path + Delete button.
 */
export function VoiceTranscriptionSection() {
  const [status, setStatus] = useState<SpeechModelStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState<ModelDownloadProgress | null>(null);
  const showToast = useStore((s) => s.showToast);
  const unlistenRef = useRef<UnlistenFn | null>(null);

  const refresh = async () => {
    try {
      const s = await api.getSpeechModelStatus();
      setStatus(s);
    } catch (e) {
      showToast("Could not read speech model status: " + String(e));
    }
  };

  useEffect(() => {
    void refresh();
    // Subscribe to download-progress events. Tauri events return an
    // unlisten function; we tear it down on unmount.
    let alive = true;
    (async () => {
      try {
        const un = await listen<ModelDownloadProgress>(
          "transcribe://model-progress",
          (e) => {
            if (alive) setProgress(e.payload);
          },
        );
        unlistenRef.current = un;
      } catch { /* listen unavailable in browser preview */ }
    })();
    return () => {
      alive = false;
      if (unlistenRef.current) {
        unlistenRef.current();
        unlistenRef.current = null;
      }
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const onDownload = async () => {
    setBusy(true);
    setProgress({ downloaded: 0, total: status?.modelSizeBytes ?? 0 });
    try {
      await api.downloadSpeechModel();
      showToast("Speech model downloaded");
      setProgress(null);
      await refresh();
    } catch (e) {
      showToast("Model download failed: " + String(e));
      setProgress(null);
    } finally {
      setBusy(false);
    }
  };

  const onDelete = async () => {
    setBusy(true);
    try {
      await api.deleteSpeechModel();
      showToast("Speech model deleted");
      await refresh();
    } catch (e) {
      showToast("Could not delete model: " + String(e));
    } finally {
      setBusy(false);
    }
  };

  const mb = (b: number) => (b / (1024 * 1024)).toFixed(1);
  const pct =
    progress && progress.total > 0
      ? Math.min(100, Math.round((progress.downloaded / progress.total) * 100))
      : null;

  return (
    <div>
      <div className="font-medium flex items-center gap-2">
        <MicVocal size={16} aria-hidden /> Voice transcription
      </div>
      <p className="text-sm text-gray-600 dark:text-gray-400 mt-1">
        Optionally transcribe voice notes to text on this device with
        whisper.cpp (same engine{" "}
        <span className="font-mono">Vibe</span> uses). The model is downloaded
        once (~{status ? mb(status.modelSizeBytes) : "57"} MB) and stays on
        your computer. After download, transcription runs <strong>fully
        offline</strong> — no audio ever leaves your machine.
      </p>

      {status === null ? (
        <div className="mt-3 text-sm text-gray-500 dark:text-gray-400 flex items-center gap-2">
          <Loader2 size={14} className="animate-spin" aria-hidden /> Checking
          model status…
        </div>
      ) : busy && progress ? (
        <div className="mt-3">
          <div className="text-sm text-gray-600 dark:text-gray-400 flex items-center gap-2">
            <Loader2 size={14} className="animate-spin" aria-hidden />
            Downloading model… {pct ?? 0}% ({mb(progress.downloaded)} /{" "}
            {mb(progress.total || status.modelSizeBytes)} MB)
          </div>
          <div
            className="mt-2 h-2 bg-gray-200 dark:bg-[#3c4043] rounded overflow-hidden"
            role="progressbar"
            aria-valuenow={pct ?? 0}
            aria-valuemin={0}
            aria-valuemax={100}
          >
            <div
              className="h-full bg-[var(--keepr-accent)] transition-all duration-200"
              style={{ width: `${pct ?? 0}%` }}
            />
          </div>
        </div>
      ) : status.downloaded ? (
        <div className="mt-3">
          <div className="text-sm text-green-700 dark:text-green-400 flex items-center gap-2">
            <CheckCircle2 size={14} aria-hidden /> Model ready —{" "}
            <span className="font-mono text-xs">{status.modelId}</span>
          </div>
          <div className="text-[11px] text-gray-500 dark:text-gray-400 mt-1 break-all font-mono">
            {status.onDiskPath}
          </div>
          <div className="flex gap-2 mt-3">
            <button
              disabled={busy}
              onClick={onDelete}
              className="flex items-center gap-2 px-3 py-2 text-sm rounded border border-gray-300 dark:border-[#5f6368] hover:bg-black/5 dark:hover:bg-white/10 disabled:opacity-50"
            >
              <Trash2 size={16} aria-hidden /> Delete model
            </button>
          </div>
        </div>
      ) : (
        <div className="mt-3 flex flex-col gap-2">
          <button
            disabled={busy}
            onClick={onDownload}
            className="self-start flex items-center gap-2 px-3 py-2 text-sm rounded text-white font-medium bg-[var(--keepr-accent)] hover:bg-[var(--keepr-accent-hover)] disabled:opacity-50"
          >
            <Download size={16} aria-hidden /> Download model (
            {mb(status.modelSizeBytes)} MB)
          </button>
          <p className="text-[11px] text-gray-500 dark:text-gray-400">
            Source: {status.modelUrl}
          </p>
        </div>
      )}
    </div>
  );
}
