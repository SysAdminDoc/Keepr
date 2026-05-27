import { useEffect, useRef, useState } from "react";
import { Mic, Square, X } from "lucide-react";

/**
 * v0.22.5 — voice note recorder. WebView2 supports MediaRecorder; we
 * record straight to `audio/webm;codecs=opus` (~12-32kbps for speech).
 * Blob bytes go to `add_audio_attachment_bytes` on the Rust side.
 *
 * v0.20.3 → v0.22.5 fix: WebView2's default `PermissionRequested`
 * handler silently *denies* mic access without showing a prompt — so
 * users saw "recording doesn't work" with no error path. Two-part fix:
 *   1. `additionalBrowserArgs: "--use-fake-ui-for-media-stream"` in
 *      tauri.conf.json auto-allows mic/camera inside our embedded
 *      WebView2 (no effect on the user's regular Edge browser).
 *   2. Defensive guards below for `navigator.mediaDevices` being
 *      undefined (some locked-down corporate WebView2 builds), and
 *      explicit `NotAllowedError` / `NotFoundError` / `OverconstrainedError`
 *      messaging so the user knows *why* it failed.
 *
 * Codec fallback chain (webm/opus → webm → mp4 → ogg) covers any
 * non-WebView2 host that might run this code (browser preview, future
 * macOS/Linux builds).
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
  // `ready` mirrors `recRef.current` for render purposes. Refs don't
  // trigger re-renders, so without this the Record button's
  // `disabled={... || !recRef.current}` would stay disabled forever
  // even after mic acquisition succeeded. (v0.22.6 regression fix.)
  const [ready, setReady] = useState(false);
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
    setReady(false);
    chunksRef.current = [];
    // Kick off mic acquisition immediately so the OS prompt appears
    // on open rather than on the Start button — saves a click and the
    // common case ("user already approved the mic earlier") goes
    // straight to ready.
    (async () => {
      // Defensive: `navigator.mediaDevices` is undefined in WebView2
      // builds with media APIs disabled, or when the page is loaded
      // over an insecure origin. Surface a clear error instead of a
      // TypeError on the next line.
      if (!navigator.mediaDevices || typeof navigator.mediaDevices.getUserMedia !== "function") {
        setError(
          "Microphone API unavailable in this WebView2 build. Update Microsoft Edge WebView2 Runtime from https://go.microsoft.com/fwlink/p/?LinkId=2124703 and relaunch Keepr.",
        );
        return;
      }
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
        setReady(true);
      } catch (e) {
        // Map well-known DOMException names to actionable text. This is
        // what users actually see when WebView2 PermissionRequested is
        // auto-denying, no physical mic exists, etc.
        const name = e instanceof DOMException ? e.name : "";
        let msg: string;
        if (name === "NotAllowedError" || name === "SecurityError") {
          msg = "Microphone access was denied. Check Windows Settings → Privacy → Microphone, ensure Keepr is allowed, then close + reopen Keepr.";
        } else if (name === "NotFoundError" || name === "OverconstrainedError") {
          msg = "No microphone was found. Plug in or enable a recording device, then try again.";
        } else if (name === "NotReadableError") {
          msg = "Microphone is in use by another app (Zoom, Teams, etc.). Close it and try again.";
        } else {
          msg = "Could not start the microphone: " + String(e);
        }
        setError(msg);
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
        ) : !ready ? (
          <p className="text-sm text-gray-600 dark:text-gray-400 mb-3">
            Acquiring microphone…
          </p>
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
              disabled={!!error || saving || !ready}
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
