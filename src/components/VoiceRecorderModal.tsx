import { useEffect, useRef, useState } from "react";
import { Mic, Square, X } from "lucide-react";

/**
 * v0.20.3 — voice note recorder. WebView2 supports MediaRecorder; we
 * record straight to `audio/webm;codecs=opus` (Opus is the smallest
 * sane codec; ~12-32kbps for speech). The blob bytes get sent to
 * `add_audio_attachment_bytes` on the Rust side which writes them
 * unchanged to `<data_dir>/resources/<id>.webm`.
 *
 * On platforms where `audio/webm` isn't available (Safari/macOS in
 * theory; not relevant for our WebView2 target but we still fall back)
 * we try `audio/mp4` then `audio/ogg`. If nothing works, the modal
 * shows an error and dismisses on Close.
 *
 * Permission prompt is the browser's default — Tauri WebView2 inherits
 * the OS microphone permission. We make a best-effort to acknowledge
 * the failure in the toast instead of leaving the modal stuck.
 */

interface Props {
  open: boolean;
  onSave: (bytes: Uint8Array, mime: string) => Promise<void>;
  onClose: () => void;
}

const CODEC_PREFERENCES = [
  "audio/webm;codecs=opus",
  "audio/webm",
  "audio/mp4",
  "audio/ogg",
];

function pickMime(): string | null {
  if (typeof MediaRecorder === "undefined") return null;
  for (const m of CODEC_PREFERENCES) {
    if (MediaRecorder.isTypeSupported(m)) return m;
  }
  return null;
}

export function VoiceRecorderModal({ open, onSave, onClose }: Props) {
  const [recording, setRecording] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [elapsed, setElapsed] = useState(0);
  const [saving, setSaving] = useState(false);
  const streamRef = useRef<MediaStream | null>(null);
  const recRef = useRef<MediaRecorder | null>(null);
  const chunksRef = useRef<Blob[]>([]);
  const elapsedTimer = useRef<number | null>(null);

  useEffect(() => {
    if (!open) return;
    setError(null);
    setElapsed(0);
    setRecording(false);
    setSaving(false);
    chunksRef.current = [];
    // Kick off mic acquisition immediately so the OS prompt appears
    // on open rather than on the Start button — saves a click and the
    // common case ("user already approved the mic earlier") goes
    // straight to ready.
    (async () => {
      try {
        const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
        streamRef.current = stream;
        const mime = pickMime();
        if (!mime) {
          setError("Audio recording is not supported in this build.");
          return;
        }
        const rec = new MediaRecorder(stream, { mimeType: mime });
        rec.ondataavailable = (e) => {
          if (e.data && e.data.size > 0) chunksRef.current.push(e.data);
        };
        recRef.current = rec;
      } catch (e) {
        setError(
          "Microphone access was denied. Enable it in Windows Settings → Privacy → Microphone, then try again."
            + " (" + String(e) + ")",
        );
      }
    })();
    return () => {
      // Cleanup on close — stop any active recording + release the mic.
      if (recRef.current && recRef.current.state !== "inactive") {
        try {
          recRef.current.stop();
        } catch {
          /* ignore */
        }
      }
      if (streamRef.current) {
        for (const t of streamRef.current.getTracks()) t.stop();
        streamRef.current = null;
      }
      recRef.current = null;
      if (elapsedTimer.current) {
        window.clearInterval(elapsedTimer.current);
        elapsedTimer.current = null;
      }
    };
  }, [open]);

  const start = () => {
    if (!recRef.current || recRef.current.state === "recording") return;
    chunksRef.current = [];
    recRef.current.start();
    setRecording(true);
    setElapsed(0);
    elapsedTimer.current = window.setInterval(() => {
      setElapsed((e) => e + 1);
    }, 1000);
  };

  const stop = async () => {
    if (!recRef.current) return;
    const rec = recRef.current;
    const mime = rec.mimeType || "audio/webm";
    // Wait for the dataavailable + stop events to fire.
    await new Promise<void>((resolve) => {
      rec.onstop = () => resolve();
      try {
        rec.stop();
      } catch {
        resolve();
      }
    });
    setRecording(false);
    if (elapsedTimer.current) {
      window.clearInterval(elapsedTimer.current);
      elapsedTimer.current = null;
    }
    const blob = new Blob(chunksRef.current, { type: mime });
    if (blob.size === 0) {
      setError("Recording was empty — nothing to save.");
      return;
    }
    setSaving(true);
    try {
      const ab = await blob.arrayBuffer();
      await onSave(new Uint8Array(ab), mime.split(";")[0]);
      onClose();
    } catch (e) {
      setError("Save failed: " + String(e));
    } finally {
      setSaving(false);
    }
  };

  if (!open) return null;

  const mmss = (s: number) =>
    `${Math.floor(s / 60).toString().padStart(2, "0")}:${(s % 60).toString().padStart(2, "0")}`;

  return (
    <div
      className="fixed inset-0 z-[55] modal-backdrop grid place-items-center p-4"
      role="dialog"
      aria-modal="true"
      aria-label="Voice note recorder"
      onClick={onClose}
    >
      <div
        className="w-full max-w-sm rounded-lg shadow-2xl border border-gray-300 dark:border-[#5f6368] bg-white dark:bg-[#2d2e30] p-5"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between mb-3">
          <h2 className="text-base font-medium">Voice note</h2>
          <button
            type="button"
            onClick={onClose}
            aria-label="Cancel"
            className="p-1 rounded hover:bg-black/5 dark:hover:bg-white/10"
          >
            <X size={16} />
          </button>
        </div>
        {error ? (
          <p className="text-sm text-red-600 dark:text-red-400 mb-3">{error}</p>
        ) : (
          <p className="text-sm text-gray-600 dark:text-gray-400 mb-3">
            Hold a quiet space, tap Record, speak, then tap Stop to attach the audio to this note.
          </p>
        )}
        <div className="flex items-center justify-center py-6 text-3xl font-mono tabular-nums">
          {mmss(elapsed)}
        </div>
        <div className="flex items-center justify-center gap-3">
          {!recording ? (
            <button
              type="button"
              onClick={start}
              disabled={!!error || saving || !recRef.current}
              className="flex items-center gap-2 px-4 py-2 rounded text-white font-medium bg-[var(--keepr-accent)] hover:bg-[var(--keepr-accent-hover)] disabled:opacity-50"
            >
              <Mic size={18} aria-hidden /> Record
            </button>
          ) : (
            <button
              type="button"
              onClick={stop}
              disabled={saving}
              className="flex items-center gap-2 px-4 py-2 rounded text-white font-medium bg-red-600 hover:bg-red-700 disabled:opacity-50"
            >
              <Square size={18} aria-hidden /> Stop &amp; save
            </button>
          )}
        </div>
        {saving && (
          <p className="text-xs text-center mt-3 text-gray-500 dark:text-gray-400">
            Saving…
          </p>
        )}
      </div>
    </div>
  );
}
