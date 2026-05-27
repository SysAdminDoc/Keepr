import { useEffect, useRef, useState } from "react";
import { Mic, Square, X } from "lucide-react";

/**
 * v0.22.7 — voice note recorder. WebView2 supports MediaRecorder; we
 * record straight to `audio/webm;codecs=opus` (~12-32kbps for speech).
 * Blob bytes go to `add_audio_attachment_bytes` on the Rust side.
 *
 * Three layered fixes shipped across v0.22.5 / v0.22.6 / v0.22.7:
 *   1. v0.22.5 — `additionalBrowserArgs: "--use-fake-ui-for-media-stream"`
 *      in tauri.conf.json so WebView2's default PermissionRequested
 *      handler doesn't auto-deny mic without prompting.
 *   2. v0.22.6 — mirror `recRef.current` into a `ready` state so the
 *      Record button actually enables after mic acquisition (useRef
 *      assignments don't trigger re-render).
 *   3. v0.22.7 — `rec.start(1000)` timeslice so ondataavailable fires
 *      every second (some Chromium builds buffer indefinitely without
 *      it and emit one massive chunk on stop — which sometimes drops on
 *      crash). Final `rec.requestData()` before stop() flushes the tail.
 *      AudioContext + AnalyserNode level meter so users can *see* if
 *      their mic is registering audio before they hit Record.
 *
 * Codec fallback chain (webm/opus → webm → mp4 → ogg) covers any
 * non-WebView2 host that might run this code.
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
  const [ready, setReady] = useState(false);
  // 0..1, smoothed RMS amplitude. Drives the level meter so users can
  // confirm their mic is actually picking up sound before recording.
  const [level, setLevel] = useState(0);
  const streamRef = useRef<MediaStream | null>(null);
  const recRef = useRef<MediaRecorder | null>(null);
  const chunksRef = useRef<Blob[]>([]);
  const elapsedTimer = useRef<number | null>(null);
  const audioCtxRef = useRef<AudioContext | null>(null);
  const analyserRef = useRef<AnalyserNode | null>(null);
  const sourceRef = useRef<MediaStreamAudioSourceNode | null>(null);
  const rafRef = useRef<number | null>(null);

  useEffect(() => {
    if (!open) return;
    setError(null);
    setElapsed(0);
    setRecording(false);
    setSaving(false);
    setReady(false);
    setLevel(0);
    chunksRef.current = [];
    (async () => {
      if (!navigator.mediaDevices || typeof navigator.mediaDevices.getUserMedia !== "function") {
        setError(
          "Microphone API unavailable in this WebView2 build. Update Microsoft Edge WebView2 Runtime from https://go.microsoft.com/fwlink/p/?LinkId=2124703 and relaunch Keepr.",
        );
        return;
      }
      try {
        // Request explicit echo/noise/AGC processing — most mic-on-laptop
        // users want this. Disabling these (constraint: false) is for
        // music recording, not voice notes.
        const stream = await navigator.mediaDevices.getUserMedia({
          audio: {
            echoCancellation: true,
            noiseSuppression: true,
            autoGainControl: true,
          },
        });
        streamRef.current = stream;

        // Sanity check: did we actually get an audio track? In some
        // WebView2 builds getUserMedia resolves with a stream that has
        // zero tracks (driver issue, mic unplugged mid-request).
        const tracks = stream.getAudioTracks();
        if (tracks.length === 0) {
          setError("Microphone returned no audio tracks. Try plugging the mic back in or rebooting.");
          return;
        }
        const tLabel = tracks[0].label || "(unnamed mic)";
        console.log("[voice] mic acquired:", tLabel, "settings=", tracks[0].getSettings?.());

        // Wire up the level meter (AudioContext → MediaStreamSource → Analyser).
        try {
          const Ctx = window.AudioContext || (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext;
          const ctx = new Ctx();
          // Some browsers start the context in "suspended" state until
          // a user gesture — the modal-open click counts but we resume
          // explicitly to be safe.
          if (ctx.state === "suspended") await ctx.resume();
          const source = ctx.createMediaStreamSource(stream);
          const analyser = ctx.createAnalyser();
          analyser.fftSize = 512;
          source.connect(analyser);
          audioCtxRef.current = ctx;
          analyserRef.current = analyser;
          sourceRef.current = source;
          const buf = new Uint8Array(analyser.fftSize);
          const tick = () => {
            if (!analyserRef.current) return;
            analyserRef.current.getByteTimeDomainData(buf);
            // RMS amplitude of the centered signal (samples are 0..255,
            // 128 is silence). Normalize to ~0..1.
            let sum = 0;
            for (let i = 0; i < buf.length; i++) {
              const v = (buf[i] - 128) / 128;
              sum += v * v;
            }
            const rms = Math.sqrt(sum / buf.length);
            // Light exponential smoothing so the meter doesn't flicker.
            setLevel((prev) => prev * 0.6 + Math.min(1, rms * 2.5) * 0.4);
            rafRef.current = requestAnimationFrame(tick);
          };
          rafRef.current = requestAnimationFrame(tick);
        } catch (meterErr) {
          // Level meter is a nice-to-have; don't block recording if it
          // fails to spin up.
          console.warn("[voice] level meter init failed:", meterErr);
        }

        const mime = pickMime();
        if (!mime) {
          setError("Audio recording is not supported in this build.");
          return;
        }
        const rec = new MediaRecorder(stream, { mimeType: mime });
        rec.ondataavailable = (e) => {
          if (e.data && e.data.size > 0) {
            chunksRef.current.push(e.data);
            console.log("[voice] chunk:", e.data.size, "bytes (total chunks:", chunksRef.current.length, ")");
          }
        };
        rec.onerror = (ev) => {
          console.error("[voice] MediaRecorder error:", ev);
          setError("Recording failed mid-capture. See dev console for details.");
        };
        recRef.current = rec;
        setReady(true);
        console.log("[voice] MediaRecorder ready with mime:", mime);
      } catch (e) {
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
      if (recRef.current && recRef.current.state !== "inactive") {
        try { recRef.current.stop(); } catch { /* ignore */ }
      }
      if (streamRef.current) {
        for (const t of streamRef.current.getTracks()) t.stop();
        streamRef.current = null;
      }
      if (rafRef.current != null) {
        cancelAnimationFrame(rafRef.current);
        rafRef.current = null;
      }
      try { sourceRef.current?.disconnect(); } catch { /* ignore */ }
      try { analyserRef.current?.disconnect(); } catch { /* ignore */ }
      if (audioCtxRef.current && audioCtxRef.current.state !== "closed") {
        audioCtxRef.current.close().catch(() => {});
      }
      sourceRef.current = null;
      analyserRef.current = null;
      audioCtxRef.current = null;
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
    // 1000 ms timeslice = ondataavailable fires every second. Without
    // a timeslice some Chromium builds buffer everything until stop()
    // and emit one huge chunk — which has occasionally been observed
    // to drop entirely.
    recRef.current.start(1000);
    setRecording(true);
    setElapsed(0);
    elapsedTimer.current = window.setInterval(() => {
      setElapsed((e) => e + 1);
    }, 1000);
    console.log("[voice] recording started, state=", recRef.current.state);
  };

  const stop = async () => {
    if (!recRef.current) return;
    const rec = recRef.current;
    const mime = rec.mimeType || "audio/webm";
    // Force a final ondataavailable for anything buffered since the
    // last timeslice tick. Safe to call even if state is "inactive".
    try {
      if (rec.state === "recording") rec.requestData();
    } catch { /* ignore */ }
    // Wait for the stop event (which is emitted *after* the final
    // ondataavailable fires).
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
    const totalBytes = chunksRef.current.reduce((acc, c) => acc + c.size, 0);
    console.log("[voice] stopped. chunks=", chunksRef.current.length, "totalBytes=", totalBytes);
    const blob = new Blob(chunksRef.current, { type: mime });
    if (blob.size === 0) {
      setError(
        "Recording captured 0 bytes — your microphone may be muted, set to a different default device, or blocked at the OS level. Try clicking the mic-level bar to confirm input, or check Windows Settings → Sound → Input.",
      );
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

  // Level meter: 20 vertical bars, each lit if the smoothed RMS exceeds
  // its threshold. Green up to ~70%, amber 70-90%, red past 90%.
  const BAR_COUNT = 20;
  const litBars = Math.round(level * BAR_COUNT);
  const meterBars = Array.from({ length: BAR_COUNT }, (_, i) => {
    const isLit = i < litBars;
    let color = "bg-gray-200 dark:bg-[#3c4043]";
    if (isLit) {
      const ratio = (i + 1) / BAR_COUNT;
      if (ratio > 0.9) color = "bg-red-500";
      else if (ratio > 0.7) color = "bg-amber-500";
      else color = "bg-green-500";
    }
    return <div key={i} className={`flex-1 ${color} transition-colors duration-75`} style={{ height: 14 }} />;
  });

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
            {recording
              ? "Recording — speak normally. Watch the level meter; tap Stop when done."
              : "Speak to test the mic — the bars below should move. Then tap Record."}
          </p>
        )}
        {ready && !error && (
          <div className="mb-3" aria-label="Microphone level">
            <div className="flex items-end gap-[2px] h-[14px]">{meterBars}</div>
            <div className="mt-1 text-[10px] uppercase tracking-wide text-gray-500 dark:text-gray-400 text-center">
              {level < 0.02
                ? "No input detected — say something"
                : level < 0.15
                ? "Low — speak up or move closer"
                : level > 0.85
                ? "Clipping — back off the mic"
                : "Good input"}
            </div>
          </div>
        )}
        <div className="flex items-center justify-center py-4 text-3xl font-mono tabular-nums">
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
