//! Offline speech-to-text for voice-note attachments.
//!
//! Pipeline:
//!   1. Read the WAV file the v0.22.9+ voice recorder saved.
//!   2. Resample 48 kHz mono i16 → 16 kHz mono f32 via `rubato` (Whisper's
//!      required input format).
//!   3. Run `whisper-rs` on the samples with the user-selected model.
//!   4. Persist the transcript text to the `transcripts` table.
//!
//! The whisper model is NOT bundled. `download_model` streams ~57 MB
//! (base.en-q5_1) from Hugging Face on first use with explicit consent.
//! SHA-256 verification against the digest baked in this file ensures
//! we don't run on tampered weights. Model lives at
//! `<app_data_dir>/models/<MODEL_FILENAME>`.
//!
//! Threading: whisper.cpp is CPU-heavy. Each transcription runs on a
//! dedicated `std::thread::spawn` to avoid blocking Tauri's command
//! handler. Results return via `tokio::sync::oneshot` so the `async fn`
//! command can `.await` it without a full tokio runtime.
//!
//! Network: outbound HTTP happens ONLY during `download_model`. After
//! the file is on disk, transcription is fully offline forever — same
//! offline-first promise as the rest of Keepr.

use anyhow::{anyhow, bail, Context, Result};
use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, Manager};

/// The single supported model for v0.23.0. Base.en is the sweet spot
/// for voice notes per the Vibe / whisper.cpp community: better than
/// tiny on proper nouns + numbers, ~3× smaller than small for marginal
/// quality loss on short clips. Quantized (q5_1) keeps the download
/// under 60 MB.
pub const MODEL_FILENAME: &str = "ggml-base.en-q5_1.bin";
pub const MODEL_BYTES: u64 = 59_721_011; // ~57.0 MiB; reported by HF
/// SHA-256 of the Git LFS object Hugging Face serves for this model.
/// If Hugging Face ever re-uploads with a different hash, this needs
/// to bump in lockstep with the URL — refusing the download is the
/// correct failure mode (don't run on weights we didn't verify).
pub const MODEL_SHA256_HEX: &str =
    "4baf70dd0d7c4247ba2b81fafd9c01005ac77c2f9ef064e00dcf195d0e2fdd2f";
pub const MODEL_URL: &str =
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en-q5_1.bin";
/// Model identifier persisted in `transcripts.model` so a future model
/// upgrade can identify stale transcripts.
pub const MODEL_ID: &str = "base.en-q5_1";

/// Subdirectory under `<app_data_dir>` where the whisper model lives.
const MODELS_DIR: &str = "models";

/// Resolve the absolute path to the model file (whether or not it
/// exists yet). Used by both the download command and the transcribe
/// command.
pub fn model_path(data_dir: &Path) -> PathBuf {
    data_dir.join(MODELS_DIR).join(MODEL_FILENAME)
}

pub fn model_present(data_dir: &Path) -> bool {
    model_path(data_dir).exists()
}

/// Verify the on-disk model file matches MODEL_SHA256_HEX.
pub fn verify_model_sha256(path: &Path) -> Result<()> {
    let mut hasher = Sha256::new();
    let mut reader = BufReader::new(File::open(path)?);
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let got = hex_lower(&hasher.finalize());
    if !got.eq_ignore_ascii_case(MODEL_SHA256_HEX) {
        bail!(
            "model SHA-256 mismatch: expected {MODEL_SHA256_HEX}, got {got}. Delete the model in Settings and download it again."
        );
    }
    Ok(())
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write;
        let _ = write!(s, "{:02x}", b);
    }
    s
}

/// Download progress event payload emitted on `transcribe://model-progress`.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelProgress {
    pub downloaded: u64,
    pub total: u64,
}

/// Stream the model file from Hugging Face into a `.partial` file beside
/// the final path; verify SHA-256 incrementally; on match, atomically
/// rename into place. Emits `transcribe://model-progress` events ~5×/sec.
///
/// Idempotent: if the model already exists and verifies, returns Ok(())
/// without touching the network.
pub async fn download_model(app: AppHandle, data_dir: PathBuf) -> Result<()> {
    let final_path = model_path(&data_dir);
    if final_path.exists() {
        // Verify in-place; if it's intact, skip the download.
        if verify_model_sha256(&final_path).is_ok() {
            log::info!("download_model: existing model SHA-256 verified, skipping download");
            return Ok(());
        }
        log::warn!("download_model: existing model failed SHA-256 verification, re-downloading");
        let _ = std::fs::remove_file(&final_path);
    }
    let models_dir = final_path.parent().unwrap();
    std::fs::create_dir_all(models_dir)
        .with_context(|| format!("create_dir_all {}", models_dir.display()))?;
    let partial = final_path.with_extension("partial");
    let _ = std::fs::remove_file(&partial); // start clean — sha is computed from scratch

    log::info!("download_model: starting {MODEL_URL} (expected SHA-256 {MODEL_SHA256_HEX})");
    let client = reqwest::Client::builder()
        .user_agent(concat!("Keepr/", env!("CARGO_PKG_VERSION")))
        .build()?;
    let resp = client.get(MODEL_URL).send().await?.error_for_status()?;
    let total = resp.content_length().unwrap_or(MODEL_BYTES);

    let mut file = BufWriter::new(File::create(&partial)?);
    let mut hasher = Sha256::new();
    let mut downloaded: u64 = 0;
    let mut last_emit: u64 = 0;
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        hasher.update(&chunk);
        file.write_all(&chunk)?;
        downloaded += chunk.len() as u64;
        // Emit every 256 KiB so the UI progress bar feels live without
        // flooding the renderer.
        if downloaded - last_emit >= 256 * 1024 {
            let _ = app.emit(
                "transcribe://model-progress",
                ModelProgress { downloaded, total },
            );
            last_emit = downloaded;
        }
    }
    file.flush()?;
    drop(file);

    let got = hex_lower(&hasher.finalize());
    if !got.eq_ignore_ascii_case(MODEL_SHA256_HEX) {
        let _ = std::fs::remove_file(&partial);
        bail!(
            "downloaded model SHA-256 mismatch: expected {MODEL_SHA256_HEX}, got {got}. The partial download was removed; try Download model again from Settings."
        );
    }
    std::fs::rename(&partial, &final_path)
        .with_context(|| format!("rename {} to {}", partial.display(), final_path.display()))?;
    // Final 100% event so the UI bar lands cleanly.
    let _ = app.emit(
        "transcribe://model-progress",
        ModelProgress {
            downloaded,
            total: downloaded,
        },
    );
    log::info!(
        "download_model: complete ({downloaded} bytes, SHA-256 {MODEL_SHA256_HEX}) -> {}",
        final_path.display(),
    );
    Ok(())
}

/// Delete the model file from disk. Idempotent. Returns Ok(()) whether
/// or not the file existed.
pub fn delete_model(data_dir: &Path) -> Result<()> {
    let p = model_path(data_dir);
    if p.exists() {
        std::fs::remove_file(&p)?;
        log::info!("delete_model: removed {}", p.display());
    }
    Ok(())
}

/// Decode a WAV file into 16 kHz mono f32 samples ready for Whisper.
/// Supports 16-bit PCM mono inputs at any sample rate (the v0.22.9
/// recorder produces 48 kHz, but we don't hardcode that — the WAV
/// header is the source of truth).
pub fn wav_to_whisper_samples(wav_path: &Path) -> Result<Vec<f32>> {
    let reader =
        hound::WavReader::open(wav_path).with_context(|| format!("open {}", wav_path.display()))?;
    let spec = reader.spec();
    if spec.channels != 1 {
        bail!("expected mono WAV, got {} channels", spec.channels);
    }
    if spec.bits_per_sample != 16 || spec.sample_format != hound::SampleFormat::Int {
        bail!(
            "expected 16-bit PCM WAV, got {}-bit {:?}",
            spec.bits_per_sample,
            spec.sample_format
        );
    }
    let source_rate = spec.sample_rate;
    // i16 PCM -> f32 in [-1, 1]
    let samples_i16: Vec<i16> = reader
        .into_samples::<i16>()
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let samples_f32: Vec<f32> = samples_i16
        .into_iter()
        .map(|s| s as f32 / 32768.0)
        .collect();

    if source_rate == 16_000 {
        return Ok(samples_f32);
    }

    // Resample to 16 kHz mono with rubato's FFT polyphase resampler.
    use rubato::{FftFixedIn, Resampler};
    let chunk_size = 1024;
    let mut resampler = FftFixedIn::<f32>::new(source_rate as usize, 16_000, chunk_size, 2, 1)?;

    let mut output: Vec<f32> =
        Vec::with_capacity(samples_f32.len() * 16_000 / source_rate as usize);
    let mut pos = 0;
    while pos + chunk_size <= samples_f32.len() {
        let in_buf = vec![samples_f32[pos..pos + chunk_size].to_vec()];
        let out_buf = resampler.process(&in_buf, None)?;
        output.extend_from_slice(&out_buf[0]);
        pos += chunk_size;
    }
    // Tail: pad the remaining samples with zeros to a full chunk so the
    // resampler can flush them.
    if pos < samples_f32.len() {
        let mut tail = samples_f32[pos..].to_vec();
        tail.resize(chunk_size, 0.0);
        let in_buf = vec![tail];
        let out_buf = resampler.process(&in_buf, None)?;
        // Only keep proportional output to avoid trailing-silence
        // padding from skewing the transcript.
        let kept = (samples_f32.len() - pos) * 16_000 / source_rate as usize;
        output.extend_from_slice(&out_buf[0][..kept.min(out_buf[0].len())]);
    }
    Ok(output)
}

/// Run whisper.cpp on a prepared 16 kHz mono f32 sample buffer. This is
/// synchronous and CPU-heavy — call it from a dedicated OS thread, never
/// from the async runtime.
pub fn transcribe_samples_blocking(model_path: &Path, samples: &[f32]) -> Result<String> {
    use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};
    let model_path_str = model_path
        .to_str()
        .ok_or_else(|| anyhow!("model path is not valid UTF-8: {}", model_path.display()))?;
    let ctx = WhisperContext::new_with_params(model_path_str, WhisperContextParameters::default())
        .map_err(|e| anyhow!("whisper context init failed: {e:?}"))?;
    let mut state = ctx
        .create_state()
        .map_err(|e| anyhow!("whisper state init failed: {e:?}"))?;

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_translate(false);
    params.set_language(Some("en"));
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    // Whisper's internal thread count — leave to library default (cores - 1).

    state
        .full(params, samples)
        .map_err(|e| anyhow!("whisper inference failed: {e:?}"))?;

    let n_segments = state.full_n_segments();
    let mut out = String::new();
    for i in 0..n_segments {
        let seg = state
            .get_segment(i)
            .ok_or_else(|| anyhow!("whisper segment {i} out of bounds"))?;
        let text = seg
            .to_str_lossy()
            .map_err(|e| anyhow!("whisper segment {i} read failed: {e:?}"))?;
        let trimmed = text.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(trimmed);
    }
    Ok(out)
}

/// CRC32 of the WAV file's bytes. Used to short-circuit a redundant
/// transcription when the user re-clicks Transcribe on an unchanged
/// audio file — the prior transcript's `source_crc32` will match.
pub fn wav_crc32(wav_path: &Path) -> Result<u32> {
    let mut hasher = crc32fast::Hasher::new();
    let mut f = BufReader::new(File::open(wav_path)?);
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize())
}

/// Resolve where Tauri stores its per-app data dir. Same resolution
/// the rest of Keepr uses — must match `resolve_data_dir` in lib.rs.
/// Exposed here for the transcription module so it can read the model
/// path without taking an AppState dependency in pure helpers.
pub fn app_data_dir(app: &AppHandle) -> Result<PathBuf> {
    app.path()
        .app_data_dir()
        .map_err(|e| anyhow!("app_data_dir resolve failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_model_sha256_reports_digest_failure_recovery() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("model.bin");
        std::fs::write(&path, b"tampered model").unwrap();

        let err = verify_model_sha256(&path).unwrap_err().to_string();

        assert!(err.contains("model SHA-256 mismatch"), "got: {err}");
        assert!(err.contains(MODEL_SHA256_HEX), "got: {err}");
        assert!(err.contains("Delete the model"), "got: {err}");
        assert!(err.contains("download it again"), "got: {err}");
    }
}
