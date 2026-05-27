import { useEffect, useRef, useState } from "react";
import { Mic, Square, X } from "lucide-react";

/**
 * v0.22.9 — voice note recorder. Final shape after the
 * v0.22.5–v0.22.8 debugging cycle:
 *
 *   capture: MediaRecorder (audio/webm;codecs=opus) — proven to deliver
 *            bytes on this WebView2 build.
 *   transcode: on stop, decode the recorded blob via
 *              AudioContext.decodeAudioData (uses the browser's built-in
 *              Opus decoder, which doesn't care about the Infinity
 *              duration that breaks `<audio src=>` playback).
 *   output:  encode the resulting AudioBuffer as 16-bit mono PCM WAV
 *            with a real sample count in the header. Plays everywhere
 *            (Chrome, Firefox, VLC, Windows Media Player) AND is the
 *            format whisper.cpp wants for v0.23.0 transcription.
 *
 * What failed along the way and why we landed here:
 *   v0.22.7 — MediaRecorder → save webm directly: bytes captured, but
 *             Chrome's `<audio src=>` refused to play (Infinity duration
 *             in the EBML header).
 *   v0.22.8 — AudioContext + ScriptProcessorNode → encode WAV directly:
 *             AnalyserNode received audio (level meter worked), but
 *             ScriptProcessorNode.onaudioprocess never fired with real
 *             samples in this WebView2 build. ScriptProcessorNode is
 *             deprecated and the browser is allowed to no-op it.
 *
 * The v0.22.9 path uses only well-supported APIs: MediaRecorder for
 * capture, decodeAudioData for the codec work, and a hand-rolled 50-line
 * WAV encoder. AudioWorklet would be the modern replacement for the
 * v0.22.8 path but requires a separately-served worklet module — not
 * worth setting up when this works.
 *
 * Cross-cycle fixes still in force:
 *   v0.22.5 — additionalBrowserArgs `--use-fake-ui-for-media-stream`
 *             keeps WebView2 from auto-denying mic without prompting.
 *   v0.22.6 — `ready` state mirrors the recorder ref so the Record
 *             button enables after acquisition (refs don't re-render).
 *   v0.22.7 — live mic-level meter (kept; AnalyserNode is reliable).
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

/** Encode a Web Audio AudioBuffer as a 16-bit mono PCM WAV. Multi-channel
 *  input is downmixed by averaging channels — voice notes don't need
 *  stereo and mono is what whisper.cpp expects anyway.
 *
 *  Exported only for unit testing; the modal calls it via the local name. */
export function encodeWavFromAudioBuffer(buffer: AudioBuffer): ArrayBuffer {
  const sampleRate = buffer.sampleRate;
  const length = buffer.length;
  // Downmix to mono.
  let mono: Float32Array;
  if (buffer.numberOfChannels === 1) {
    mono = buffer.getChannelData(0);
  } else {
    mono = new Float32Array(length);
    const channels: Float32Array[] = [];
    for (let c = 0; c < buffer.numberOfChannels; c++) {
      channels.push(buffer.getChannelData(c));
    }
    for (let i = 0; i < length; i++) {
      let sum = 0;
      for (let c = 0; c < channels.length; c++) sum += channels[c][i];
      mono[i] = sum / channels.length;
    }
  }
  return encodePcmWav(mono, sampleRate);
}

/** Inner helper: encode raw Float32 PCM samples as a 16-bit mono WAV.
 *  Exported only for unit testing. */
export function encodePcmWav(samples: Float32Array, sampleRate: number): ArrayBuffer {
  const byteLength = 44 + samples.length * 2;
  const buf = new ArrayBuffer(byteLength);
  const view = new DataView(buf);
  const writeAscii = (offset: number, s: string) => {
    for (let i = 0; i < s.length; i++) view.setUint8(offset + i, s.charCodeAt(i));
  };

  writeAscii(0, "RIFF");
  view.setUint32(4, byteLength - 8, true);
  writeAscii(8, "WAVE");

  writeAscii(12, "fmt ");
  view.setUint32(16, 16, true);
  view.setUint16(20, 1, true);  // PCM
  view.setUint16(22, 1, true);  // mono
  view.setUint32(24, sampleRate, true);
  view.setUint32(28, sampleRate * 2, true); // byte rate (mono, 2 bytes/sample)
  view.setUint16(32, 2, true);  // block align
  view.setUint16(34, 16, true); // bits per sample

  writeAscii(36, "data");
  view.setUint32(40, samples.length * 2, true);

  let offset = 44;
  for (let i = 0; i < samples.length; i++) {
    const s = Math.max(-1, Math.min(1, samples[i]));
    view.setInt16(offset, s < 0 ? Math.round(s * 0x8000) : Math.round(s * 0x7fff), true);
    offset += 2;
  }
  return buf;
}

export function VoiceRecorderModal({ open, onSave, onClose }: Props) {
  const [recording, setRecording] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [elapsed, setElapsed] = useState(0);
  const [saving, setSaving] = useState(false);
  const [ready, setReady] = useState(false);
  const [level, setLevel] = useState(0);

  const streamRef = useRef<MediaStream | null>(null);
  const audioCtxRef = useRef<AudioContext | null>(null);
  const sourceRef = useRef<MediaStreamAudioSourceNode | null>(null);
  const analyserRef = useRef<AnalyserNode | null>(null);
  const recRef = useRef<MediaRecorder | null>(null);
  const chunksRef = useRef<Blob[]>([]);
  const elapsedTimer = useRef<number | null>(null);
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
        const stream = await navigator.mediaDevices.getUserMedia({
          audio: {
            echoCancellation: true,
            noiseSuppression: true,
            autoGainControl: true,
          },
        });
        streamRef.current = stream;

        const tracks = stream.getAudioTracks();
        if (tracks.length === 0) {
          setError("Microphone returned no audio tracks. Try plugging the mic back in or rebooting.");
          return;
        }
        console.log("[voice] mic acquired:", tracks[0].label || "(unnamed)", tracks[0].getSettings?.());

        const Ctx = window.AudioContext || (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext;
        const ctx = new Ctx();
        if (ctx.state === "suspended") await ctx.resume();
        audioCtxRef.current = ctx;

        // Level meter only — capture happens via MediaRecorder, not via
        // this AudioContext. The AnalyserNode is purely for the UI.
        const source = ctx.createMediaStreamSource(stream);
        const analyser = ctx.createAnalyser();
        analyser.fftSize = 512;
        source.connect(analyser);
        sourceRef.current = source;
        analyserRef.current = analyser;
        const meterBuf = new Uint8Array(analyser.fftSize);
        const tick = () => {
          if (!analyserRef.current) return;
          analyserRef.current.getByteTimeDomainData(meterBuf);
          let sum = 0;
          for (let i = 0; i < meterBuf.length; i++) {
            const v = (meterBuf[i] - 128) / 128;
            sum += v * v;
          }
          const rms = Math.sqrt(sum / meterBuf.length);
          setLevel((prev) => prev * 0.6 + Math.min(1, rms * 2.5) * 0.4);
          rafRef.current = requestAnimationFrame(tick);
        };
        rafRef.current = requestAnimationFrame(tick);

        const mime = pickMime();
        if (!mime) {
          setError("Audio recording is not supported in this build.");
          return;
        }
        const rec = new MediaRecorder(stream, { mimeType: mime });
        rec.ondataavailable = (e) => {
          if (e.data && e.data.size > 0) {
            chunksRef.current.push(e.data);
            console.log("[voice] chunk:", e.data.size, "bytes (chunks:", chunksRef.current.length, ")");
          }
        };
        rec.onerror = (ev) => {
          console.error("[voice] MediaRecorder error:", ev);
          setError("Recording failed mid-capture. See dev console for details.");
        };
        recRef.current = rec;
        setReady(true);
        console.log("[voice] MediaRecorder ready, mime:", mime, "ctx sampleRate:", ctx.sampleRate);
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
      recRef.current = null;
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
      if (streamRef.current) {
        for (const t of streamRef.current.getTracks()) t.stop();
        streamRef.current = null;
      }
      if (elapsedTimer.current) {
        window.clearInterval(elapsedTimer.current);
        elapsedTimer.current = null;
      }
    };
  }, [open]);

  const start = () => {
    if (!ready || recording) return;
    if (!recRef.current) return;
    chunksRef.current = [];
    // 1000 ms timeslice: ondataavailable fires every second so chunks
    // accumulate progressively instead of one big chunk on stop.
    recRef.current.start(1000);
    setRecording(true);
    setElapsed(0);
    elapsedTimer.current = window.setInterval(() => {
      setElapsed((e) => e + 1);
    }, 1000);
    console.log("[voice] recording started, state=", recRef.current.state);
  };

  const stop = async () => {
    if (!recording || !recRef.current) return;
    const rec = recRef.current;
    const mime = rec.mimeType || "audio/webm";
    // Flush any data buffered since the last timeslice tick.
    try { if (rec.state === "recording") rec.requestData(); } catch { /* ignore */ }
    await new Promise<void>((resolve) => {
      rec.onstop = () => resolve();
      try { rec.stop(); } catch { resolve(); }
    });
    setRecording(false);
    if (elapsedTimer.current) {
      window.clearInterval(elapsedTimer.current);
      elapsedTimer.current = null;
    }

    const totalBytes = chunksRef.current.reduce((acc, c) => acc + c.size, 0);
    console.log("[voice] stopped. chunks=", chunksRef.current.length, "totalBytes=", totalBytes, "mime=", mime);
    if (totalBytes === 0) {
      setError(
        "Recording captured 0 bytes. Check the mic-level bar — it should move when you speak.",
      );
      return;
    }

    setSaving(true);
    try {
      const recordedBlob = new Blob(chunksRef.current, { type: mime });
      const arrayBuf = await recordedBlob.arrayBuffer();
      console.log("[voice] decoding", arrayBuf.byteLength, "bytes...");

      // Decode the recorded webm/opus into PCM via the browser's built-in
      // codec — sidesteps the Infinity-duration problem that breaks
      // `<audio src=>` playback. We create a fresh AudioContext for the
      // decode since the modal's main context will be closed in cleanup.
      const decodeCtx = new (window.AudioContext || (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext)();
      let audioBuffer: AudioBuffer;
      try {
        audioBuffer = await decodeCtx.decodeAudioData(arrayBuf);
      } finally {
        await decodeCtx.close().catch(() => {});
      }
      console.log("[voice] decoded:", audioBuffer.duration.toFixed(2), "s,", audioBuffer.sampleRate, "Hz,", audioBuffer.numberOfChannels, "ch");

      const wavBytes = encodeWavFromAudioBuffer(audioBuffer);
      console.log("[voice] encoded WAV:", wavBytes.byteLength, "bytes");

      await onSave(new Uint8Array(wavBytes), "audio/wav");
      onClose();
    } catch (e) {
      console.error("[voice] transcode/save failed:", e);
      setError("Could not finalize the recording: " + String(e));
    } finally {
      setSaving(false);
    }
  };

  if (!open) return null;

  const mmss = (s: number) =>
    `${Math.floor(s / 60).toString().padStart(2, "0")}:${(s % 60).toString().padStart(2, "0")}`;

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
            Transcoding to WAV…
          </p>
        )}
      </div>
    </div>
  );
}
