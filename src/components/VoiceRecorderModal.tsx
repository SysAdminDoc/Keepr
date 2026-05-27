import { useEffect, useRef, useState } from "react";
import { Mic, Square, X } from "lucide-react";

/**
 * v0.22.8 — voice note recorder.
 *
 * Records via AudioContext + ScriptProcessorNode and encodes a real PCM
 * WAV file inline. We tried MediaRecorder (audio/webm;codecs=opus) up
 * through v0.22.7 — recording worked, the file was saved with audio
 * data, but Chrome's `<audio src=>` element refused to play it back.
 * Root cause: MediaRecorder writes the WebM EBML header with
 * Duration=Infinity because it doesn't know the recording length in
 * advance; even with the trailing element written on stop(), Chrome's
 * direct-load audio path can't determine the playable range without
 * an explicit duration in the header.
 *
 * WAV avoids all of it: the 44-byte header carries the sample count
 * (= explicit duration), 16-bit PCM mono plays in every browser and
 * media player, and it's exactly the format whisper.cpp wants for the
 * v0.23.0 transcription work — so we're not paying a switching cost
 * later. Trade-off is file size: ~96 KB/sec at 48 kHz mono 16-bit, so
 * a 1-minute voice note is ~5.5 MB. That's acceptable for personal
 * notes (and tiny vs. the rest of a typical attachment-bearing note).
 *
 * Stack:
 *   stream → MediaStreamSource → splits to:
 *     - AnalyserNode  → byte-time-domain RMS → level meter (60fps)
 *     - ScriptProcessor → captures Float32 PCM into samplesRef
 *   on stop → flatten samples → encode WAV → onSave bytes + audio/wav
 *
 * ScriptProcessorNode is deprecated in favor of AudioWorklet, but is
 * still present in every Chromium build (including WebView2). We can
 * migrate later; for now this avoids the worklet's module-loading
 * setup. The processor MUST be connected to ctx.destination for its
 * onaudioprocess to fire in Chrome — even though we don't want
 * playback. We compensate by zeroing the output buffer.
 *
 * Three earlier-cycle fixes still in force:
 *   v0.22.5 — additionalBrowserArgs `--use-fake-ui-for-media-stream`
 *             keeps WebView2 from auto-denying mic without prompting.
 *   v0.22.6 — `ready` state mirrors the imperative ref so the Record
 *             button actually enables after acquisition.
 *   v0.22.7 — live mic-level meter (kept; powered by AnalyserNode).
 */

interface Props {
  open: boolean;
  onSave: (bytes: Uint8Array, mime: string) => Promise<void>;
  onClose: () => void;
}

/** Encode interleaved Float32 PCM as a 16-bit mono WAV. */
function encodeWav(samples: Float32Array, sampleRate: number): ArrayBuffer {
  const byteLength = 44 + samples.length * 2;
  const buf = new ArrayBuffer(byteLength);
  const view = new DataView(buf);
  const writeAscii = (offset: number, s: string) => {
    for (let i = 0; i < s.length; i++) view.setUint8(offset + i, s.charCodeAt(i));
  };

  // RIFF / WAVE header
  writeAscii(0, "RIFF");
  view.setUint32(4, byteLength - 8, true); // file size minus "RIFF" + size
  writeAscii(8, "WAVE");

  // fmt sub-chunk
  writeAscii(12, "fmt ");
  view.setUint32(16, 16, true); // PCM fmt chunk size
  view.setUint16(20, 1, true);  // format = PCM (uncompressed)
  view.setUint16(22, 1, true);  // mono
  view.setUint32(24, sampleRate, true);
  view.setUint32(28, sampleRate * 2, true); // byte rate (sampleRate * channels * bytesPerSample)
  view.setUint16(32, 2, true);  // block align (channels * bytesPerSample)
  view.setUint16(34, 16, true); // bits per sample

  // data sub-chunk
  writeAscii(36, "data");
  view.setUint32(40, samples.length * 2, true);

  // PCM samples — clamp Float32 [-1, 1] and convert to little-endian Int16.
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
  const processorRef = useRef<ScriptProcessorNode | null>(null);
  // Float32 sample chunks accumulated by ScriptProcessor while recording.
  const samplesRef = useRef<Float32Array[]>([]);
  // Mirror of `recording` for the audio thread — onaudioprocess fires
  // on a non-React thread and can't read state directly without going
  // stale; this ref is updated synchronously by start/stop.
  const recordingFlagRef = useRef(false);
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
    samplesRef.current = [];
    recordingFlagRef.current = false;

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
        console.log("[voice] AudioContext sampleRate:", ctx.sampleRate, "state:", ctx.state);

        const source = ctx.createMediaStreamSource(stream);
        sourceRef.current = source;

        // Level meter branch.
        const analyser = ctx.createAnalyser();
        analyser.fftSize = 512;
        source.connect(analyser);
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

        // PCM capture branch via ScriptProcessor. Buffer size 4096 ≈ 85ms
        // at 48 kHz — good latency/efficiency balance. Mono in/out.
        const processor = ctx.createScriptProcessor(4096, 1, 1);
        processor.onaudioprocess = (e) => {
          // Zero the output so the connection-to-destination requirement
          // doesn't leak audio back to the speakers.
          const out = e.outputBuffer.getChannelData(0);
          for (let i = 0; i < out.length; i++) out[i] = 0;
          if (!recordingFlagRef.current) return;
          const input = e.inputBuffer.getChannelData(0);
          // Clone — ScriptProcessor reuses the buffer for the next tick.
          samplesRef.current.push(new Float32Array(input));
        };
        source.connect(processor);
        processor.connect(ctx.destination); // required for onaudioprocess to fire
        processorRef.current = processor;

        setReady(true);
        console.log("[voice] ready — PCM capture armed");
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
      recordingFlagRef.current = false;
      if (rafRef.current != null) {
        cancelAnimationFrame(rafRef.current);
        rafRef.current = null;
      }
      try { processorRef.current?.disconnect(); } catch { /* ignore */ }
      try { sourceRef.current?.disconnect(); } catch { /* ignore */ }
      try { analyserRef.current?.disconnect(); } catch { /* ignore */ }
      if (audioCtxRef.current && audioCtxRef.current.state !== "closed") {
        audioCtxRef.current.close().catch(() => {});
      }
      processorRef.current = null;
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
    samplesRef.current = [];
    recordingFlagRef.current = true;
    setRecording(true);
    setElapsed(0);
    elapsedTimer.current = window.setInterval(() => {
      setElapsed((e) => e + 1);
    }, 1000);
    console.log("[voice] recording started");
  };

  const stop = async () => {
    if (!recording) return;
    recordingFlagRef.current = false;
    setRecording(false);
    if (elapsedTimer.current) {
      window.clearInterval(elapsedTimer.current);
      elapsedTimer.current = null;
    }
    // Wait one audio tick so any in-flight processor callback completes
    // before we flatten the buffer.
    await new Promise((r) => setTimeout(r, 100));

    const ctx = audioCtxRef.current;
    if (!ctx) {
      setError("Audio context disappeared mid-recording.");
      return;
    }
    const totalSamples = samplesRef.current.reduce((acc, s) => acc + s.length, 0);
    console.log("[voice] stopped. chunks=", samplesRef.current.length, "samples=", totalSamples, "≈", (totalSamples / ctx.sampleRate).toFixed(2), "s");
    if (totalSamples === 0) {
      setError(
        "Recording captured 0 samples — your microphone may be muted or set to a different default device. Check the mic-level bar above; it should move when you speak.",
      );
      return;
    }

    // Flatten Float32 chunks into one continuous buffer.
    const merged = new Float32Array(totalSamples);
    let offset = 0;
    for (const chunk of samplesRef.current) {
      merged.set(chunk, offset);
      offset += chunk.length;
    }

    setSaving(true);
    try {
      const wav = encodeWav(merged, ctx.sampleRate);
      console.log("[voice] WAV encoded, bytes=", wav.byteLength);
      await onSave(new Uint8Array(wav), "audio/wav");
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
