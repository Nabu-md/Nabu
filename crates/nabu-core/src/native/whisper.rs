//! Local Whisper speech-to-text engine.
//!
//! Uses `whisper-rs` (whisper.cpp) — fully local, no network, no provider
//! abstraction. Audio input is decoded from WAV bytes into 16 kHz mono f32
//! PCM, which is what whisper.cpp expects.

use std::path::Path;

use super::error::NativeError;

/// A single transcribed segment with its time range in milliseconds.
#[derive(Debug, Clone, PartialEq)]
pub struct TranscriptionSegment {
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
}

/// The result of transcribing an audio recording.
#[derive(Debug, Clone, PartialEq)]
pub struct Transcription {
    pub text: String,
    pub confidence: f64,
    pub duration_ms: u64,
    pub segments: Vec<TranscriptionSegment>,
}

/// Transcribe WAV audio bytes with the local whisper model at `model_path`.
///
/// `model_path` points to a whisper.cpp `.bin` model file. Returns
/// [`NativeError::ModelNotFound`] when the model is absent and
/// [`NativeError::UnsupportedAudio`] for non-WAV input.
pub fn transcribe(model_path: &Path, audio_data: &[u8]) -> Result<Transcription, NativeError> {
    imp::transcribe(model_path, audio_data)
}

/// Decode WAV bytes (16-bit PCM or 32-bit float) into 16 kHz mono f32 PCM.
///
/// Pure Rust — no platform dependencies — so it is available and testable on
/// every platform.
pub fn decode_wav(data: &[u8]) -> Result<Vec<f32>, NativeError> {
    if data.len() < 12 || &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
        return Err(NativeError::UnsupportedAudio("not a RIFF/WAVE file".into()));
    }

    let mut offset = 12usize;
    let mut fmt: Option<(u16, u16, u32, u16)> = None; // (format, channels, rate, bits)
    let mut pcm: Option<&[u8]> = None;

    while offset + 8 <= data.len() {
        let id = &data[offset..offset + 4];
        let size = u32::from_le_bytes([
            data[offset + 4],
            data[offset + 5],
            data[offset + 6],
            data[offset + 7],
        ]) as usize;
        let body_start = offset + 8;
        let body_end = (body_start + size).min(data.len());
        let body = &data[body_start..body_end];

        match id {
            b"fmt " if body.len() >= 16 => {
                let format = u16::from_le_bytes([body[0], body[1]]);
                let channels = u16::from_le_bytes([body[2], body[3]]);
                let rate = u32::from_le_bytes([body[4], body[5], body[6], body[7]]);
                let bits = u16::from_le_bytes([body[14], body[15]]);
                fmt = Some((format, channels, rate, bits));
            }
            b"data" => pcm = Some(body),
            _ => {}
        }
        offset = body_end + (size % 2); // chunks are word-aligned
    }

    let (format, channels, rate, bits) =
        fmt.ok_or_else(|| NativeError::UnsupportedAudio("missing fmt chunk".into()))?;
    let pcm = pcm.ok_or_else(|| NativeError::UnsupportedAudio("missing data chunk".into()))?;
    if channels == 0 || rate == 0 {
        return Err(NativeError::UnsupportedAudio("invalid fmt header".into()));
    }

    let samples = match (format, bits) {
        (1, 16) => decode_pcm16(pcm, channels),
        (3, 32) => decode_float32(pcm, channels),
        (format, bits) => {
            return Err(NativeError::UnsupportedAudio(format!(
                "format {format}/{bits}-bit is not supported; use 16-bit PCM or 32-bit float WAV"
            )));
        }
    };

    let samples = if channels > 1 {
        // Downmix multi-channel to mono by averaging channels per frame.
        let frame_len = samples.len() / channels as usize;
        (0..frame_len)
            .map(|f| {
                let sum: f32 = (0..channels as usize)
                    .map(|c| samples[f * channels as usize + c])
                    .sum();
                sum / channels as f32
            })
            .collect::<Vec<_>>()
    } else {
        samples
    };

    Ok(resample_to_16k(&samples, rate))
}

fn decode_pcm16(pcm: &[u8], _channels: u16) -> Vec<f32> {
    pcm.chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0)
        .collect()
}

fn decode_float32(pcm: &[u8], _channels: u16) -> Vec<f32> {
    pcm.chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

fn resample_to_16k(samples: &[f32], rate: u32) -> Vec<f32> {
    const TARGET: u32 = 16_000;
    if rate == TARGET || samples.is_empty() {
        return samples.to_vec();
    }
    let ratio = rate as f64 / TARGET as f64;
    let out_len = (samples.len() as f64 / ratio) as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src = i as f64 * ratio;
        let idx = src.floor() as usize;
        let frac = src - idx as f64;
        let a = samples[idx.min(samples.len() - 1)];
        let b = samples[(idx + 1).min(samples.len() - 1)];
        out.push((a as f64 * (1.0 - frac) + b as f64 * frac) as f32);
    }
    out
}

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

    pub fn transcribe(model_path: &Path, audio_data: &[u8]) -> Result<Transcription, NativeError> {
        if !model_path.exists() {
            return Err(NativeError::ModelNotFound(model_path.display().to_string()));
        }
        let path_str = model_path
            .to_str()
            .ok_or_else(|| NativeError::InvalidData("model path is not valid UTF-8".into()))?;

        let samples = decode_wav(audio_data)?;

        let ctx = WhisperContext::new_with_params(path_str, WhisperContextParameters::default())
            .map_err(|e| NativeError::CallFailed(format!("could not load whisper model: {e}")))?;
        let mut state = ctx
            .create_state()
            .map_err(|e| NativeError::CallFailed(format!("could not init whisper state: {e}")))?;

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_n_threads(4);
        params.set_language(Some("en"));
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);

        let n_segments = state
            .full(params, &samples)
            .map_err(|e| NativeError::CallFailed(format!("whisper transcription failed: {e}")))?;

        let mut segments = Vec::with_capacity(n_segments.max(0) as usize);
        let mut total_prob = 0.0f64;
        let mut token_count = 0usize;

        for i in 0..n_segments {
            let start_ms = state.full_get_segment_t0(i).unwrap_or(0);
            let end_ms = state.full_get_segment_t1(i).unwrap_or(start_ms);
            let text = state.full_get_segment_text_lossy(i).unwrap_or_default();
            // Average token probability for a real confidence estimate.
            if let Ok(n_tokens) = state.full_n_tokens(i) {
                for t in 0..n_tokens {
                    if let Ok(p) = state.full_get_token_prob(i, t) {
                        total_prob += p as f64;
                        token_count += 1;
                    }
                }
            }
            segments.push(TranscriptionSegment {
                start_ms,
                end_ms,
                text: text.trim().to_string(),
            });
        }

        let duration_ms = segments.last().map(|s| s.end_ms.max(0) as u64).unwrap_or(0);
        let confidence = if token_count > 0 {
            total_prob / token_count as f64
        } else {
            0.0
        };
        let text = segments
            .iter()
            .map(|s| s.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
            .trim()
            .to_string();

        Ok(Transcription {
            text,
            confidence,
            duration_ms,
            segments,
        })
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::*;

    pub fn transcribe(
        _model_path: &Path,
        _audio_data: &[u8],
    ) -> Result<Transcription, NativeError> {
        Err(NativeError::UnsupportedPlatform)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine_wav(rate: u32, channels: u16, seconds: u32) -> Vec<u8> {
        let n = rate * seconds;
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        let data_len = n * channels as u32 * 2;
        wav.extend_from_slice(&(36 + data_len).to_le_bytes());
        wav.extend_from_slice(b"WAVE");
        wav.extend_from_slice(b"fmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
        wav.extend_from_slice(&channels.to_le_bytes());
        wav.extend_from_slice(&rate.to_le_bytes());
        wav.extend_from_slice(&(rate * channels as u32 * 2).to_le_bytes());
        wav.extend_from_slice(&(channels * 2).to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_len.to_le_bytes());
        for i in 0..n {
            let sample = (i as f32 * 440.0 * 2.0 * std::f32::consts::PI / rate as f32).sin();
            let v = (sample * 0.5 * i16::MAX as f32) as i16;
            for _ in 0..channels {
                wav.extend_from_slice(&v.to_le_bytes());
            }
        }
        wav
    }

    #[test]
    fn decodes_16k_mono_wav() {
        let wav = sine_wav(16_000, 1, 1);
        let samples = decode_wav(&wav).expect("decode should succeed");
        assert_eq!(samples.len(), 16_000);
        assert!(samples.iter().any(|s| s.abs() > 0.1));
    }

    #[test]
    fn decodes_and_downmixes_stereo() {
        let wav = sine_wav(16_000, 2, 1);
        let samples = decode_wav(&wav).expect("decode should succeed");
        assert_eq!(samples.len(), 16_000); // downmixed to mono
    }

    #[test]
    fn resamples_48k_to_16k() {
        let wav = sine_wav(48_000, 1, 1);
        let samples = decode_wav(&wav).expect("decode should succeed");
        assert_eq!(samples.len(), 16_000);
    }

    #[test]
    fn rejects_non_wav() {
        assert!(decode_wav(b"not a wav file at all").is_err());
    }
}
