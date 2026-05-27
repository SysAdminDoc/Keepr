import { describe, expect, it } from "vitest";
import { encodePcmWav, encodeWavFromAudioBuffer } from "../components/VoiceRecorderModal";

/**
 * v0.22.10 — coverage for the v0.22.9 WAV encoder, the critical-path
 * function that turns the decoded `AudioBuffer` from `decodeAudioData`
 * into a 16-bit mono PCM WAV that plays in the `<audio>` element AND
 * is exactly what whisper.cpp wants for the planned v0.23.0
 * transcription work. Without these tests the encoder was a black box;
 * if it ever started writing a malformed header (wrong endian, off-by-
 * one on the data chunk size, etc.) audio playback would silently
 * break and we'd ship a third "voice notes don't work" cycle.
 */

function readU32LE(view: DataView, offset: number): number {
  return view.getUint32(offset, true);
}
function readU16LE(view: DataView, offset: number): number {
  return view.getUint16(offset, true);
}
function readAscii(view: DataView, offset: number, len: number): string {
  let s = "";
  for (let i = 0; i < len; i++) s += String.fromCharCode(view.getUint8(offset + i));
  return s;
}

describe("encodePcmWav (raw samples)", () => {
  it("writes a valid 44-byte WAV header for a 1-sample buffer", () => {
    const wav = encodePcmWav(new Float32Array([0]), 48000);
    expect(wav.byteLength).toBe(44 + 2); // header + 1 sample (16-bit)
    const view = new DataView(wav);
    expect(readAscii(view, 0, 4)).toBe("RIFF");
    expect(readU32LE(view, 4)).toBe(wav.byteLength - 8);
    expect(readAscii(view, 8, 4)).toBe("WAVE");
    expect(readAscii(view, 12, 4)).toBe("fmt ");
    expect(readU32LE(view, 16)).toBe(16); // PCM fmt chunk size
    expect(readU16LE(view, 20)).toBe(1);  // PCM format
    expect(readU16LE(view, 22)).toBe(1);  // mono
    expect(readU32LE(view, 24)).toBe(48000);
    expect(readU32LE(view, 28)).toBe(48000 * 2); // byte rate
    expect(readU16LE(view, 32)).toBe(2);  // block align
    expect(readU16LE(view, 34)).toBe(16); // bits per sample
    expect(readAscii(view, 36, 4)).toBe("data");
    expect(readU32LE(view, 40)).toBe(2);  // data chunk byte count
  });

  it("encodes 1 second of silence at 16 kHz", () => {
    const samples = new Float32Array(16000); // all zeros
    const wav = encodePcmWav(samples, 16000);
    expect(wav.byteLength).toBe(44 + 16000 * 2);
    const view = new DataView(wav);
    expect(readU32LE(view, 24)).toBe(16000);
    expect(readU32LE(view, 40)).toBe(16000 * 2);
    // First PCM sample should be 0x0000 (silence).
    expect(view.getInt16(44, true)).toBe(0);
  });

  it("clamps samples above +1.0 to int16 max", () => {
    const wav = encodePcmWav(new Float32Array([2.0, -2.0, 0.5, -0.5]), 48000);
    const view = new DataView(wav);
    // +1.0 → 0x7FFF (32767); -1.0 → 0x8000 (-32768 in signed).
    expect(view.getInt16(44, true)).toBe(32767);
    expect(view.getInt16(46, true)).toBe(-32768);
    // 0.5 * 0x7FFF = 16383 (rounded).
    expect(view.getInt16(48, true)).toBe(Math.round(0.5 * 0x7fff));
    // -0.5 * 0x8000 = -16384.
    expect(view.getInt16(50, true)).toBe(Math.round(-0.5 * 0x8000));
  });

  it("preserves signedness and endianness for a known waveform", () => {
    // ±1.0, ±0.5, 0 — verify each samples round-trips correctly.
    const samples = new Float32Array([1, 0, -1, 0.5, -0.5]);
    const wav = encodePcmWav(samples, 48000);
    const view = new DataView(wav);
    expect(view.getInt16(44, true)).toBe(32767);
    expect(view.getInt16(46, true)).toBe(0);
    expect(view.getInt16(48, true)).toBe(-32768);
    expect(view.getInt16(50, true)).toBe(Math.round(0.5 * 0x7fff));
    expect(view.getInt16(52, true)).toBe(Math.round(-0.5 * 0x8000));
  });

  it("handles an empty sample buffer without producing garbage", () => {
    const wav = encodePcmWav(new Float32Array(0), 48000);
    expect(wav.byteLength).toBe(44);
    const view = new DataView(wav);
    expect(readU32LE(view, 4)).toBe(36); // RIFF size = header - 8
    expect(readU32LE(view, 40)).toBe(0); // data chunk = 0 bytes
  });
});

describe("encodeWavFromAudioBuffer (downmix path)", () => {
  // Minimal AudioBuffer shim — encodeWavFromAudioBuffer only touches
  // `sampleRate`, `length`, `numberOfChannels`, and `getChannelData(i)`.
  function fakeAudioBuffer(
    sampleRate: number,
    channels: Float32Array[],
  ): AudioBuffer {
    return {
      sampleRate,
      length: channels[0]?.length ?? 0,
      numberOfChannels: channels.length,
      getChannelData: (i: number) => channels[i],
    } as unknown as AudioBuffer;
  }

  it("passes mono input through unchanged", () => {
    const buf = fakeAudioBuffer(48000, [new Float32Array([1, 0, -1])]);
    const wav = encodeWavFromAudioBuffer(buf);
    const view = new DataView(wav);
    expect(view.getInt16(44, true)).toBe(32767);
    expect(view.getInt16(46, true)).toBe(0);
    expect(view.getInt16(48, true)).toBe(-32768);
  });

  it("averages stereo to mono", () => {
    // L=1, R=-1 → averaged = 0.
    // L=0.5, R=0.5 → averaged = 0.5.
    const buf = fakeAudioBuffer(48000, [
      new Float32Array([1, 0.5]),
      new Float32Array([-1, 0.5]),
    ]);
    const wav = encodeWavFromAudioBuffer(buf);
    const view = new DataView(wav);
    expect(view.getInt16(44, true)).toBe(0);
    expect(view.getInt16(46, true)).toBe(Math.round(0.5 * 0x7fff));
  });

  it("propagates the AudioContext sample rate into the header", () => {
    const buf = fakeAudioBuffer(44100, [new Float32Array([0, 0])]);
    const wav = encodeWavFromAudioBuffer(buf);
    const view = new DataView(wav);
    expect(readU32LE(view, 24)).toBe(44100);
    expect(readU32LE(view, 28)).toBe(44100 * 2);
  });

  it("downmixes 4-channel input by averaging", () => {
    // (1 + 1 + 1 + 1) / 4 = 1.0 → +int16 max.
    const buf = fakeAudioBuffer(48000, [
      new Float32Array([1]),
      new Float32Array([1]),
      new Float32Array([1]),
      new Float32Array([1]),
    ]);
    const wav = encodeWavFromAudioBuffer(buf);
    const view = new DataView(wav);
    expect(view.getInt16(44, true)).toBe(32767);
  });
});
