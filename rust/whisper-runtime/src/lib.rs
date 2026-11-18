use std::error::Error;
use std::ffi::{CStr, CString};
use std::io::{self, Read, Seek};
use std::path::{Path, PathBuf};
use std::ptr::NonNull;
use whisper_rs_sys as whisper;

pub mod segments;
pub mod services;
pub mod audio;
pub mod models;
pub mod pipeline;
pub mod config;
pub mod server;

pub type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Clone, Debug)]
pub struct WhisperConfig {
    pub model_path: PathBuf,
    pub vad_model_path: PathBuf,
    pub language: Option<String>,
    pub request_cuda: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CudaResolution {
    pub use_cuda: bool,
    pub warning: Option<&'static str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Segment {
    pub index: usize,
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
}

fn centiseconds_to_ms(timestamp: i64) -> i64 {
    timestamp * 10
}

fn validate_samples(samples: &[f32]) -> Result<i32> {
    if samples.is_empty() || samples.iter().any(|sample| !sample.is_finite()) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid audio samples").into());
    }
    Ok(i32::try_from(samples.len())?)
}

pub fn resolve_cuda(requested: bool, cuda_compiled: bool) -> CudaResolution {
    if requested && !cuda_compiled {
        return CudaResolution {
            use_cuda: false,
            warning: Some("CUDA was requested, but this build has no CUDA support; using CPU."),
        };
    }

    CudaResolution {
        use_cuda: requested,
        warning: None,
    }
}

pub fn read_pcm_16k_wav(path: impl AsRef<Path>) -> Result<Vec<f32>> {
    read_pcm_16k_wav_reader(hound::WavReader::open(path)?)
}

fn read_pcm_16k_wav_reader<R: Read + Seek>(mut reader: hound::WavReader<R>) -> Result<Vec<f32>> {
    let spec = reader.spec();
    if spec.channels != 1
        || spec.sample_rate != 16_000
        || spec.bits_per_sample != 16
        || spec.sample_format != hound::SampleFormat::Int
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "expected mono 16 kHz 16-bit PCM WAV",
        )
        .into());
    }

    let samples = reader
        .samples::<i16>()
        .map(|sample| sample.map(|value| f32::from(value) / 32_768.0))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(samples)
}

pub struct WhisperRuntime {
    context: NonNull<whisper::whisper_context>,
    vad_model_path: CString,
    language: Option<CString>,
}

impl Drop for WhisperRuntime {
    fn drop(&mut self) {
        unsafe { whisper::whisper_free(self.context.as_ptr()) }
    }
}

impl WhisperRuntime {
    pub fn new(config: WhisperConfig) -> Result<(Self, CudaResolution)> {
        let cuda = resolve_cuda(config.request_cuda, cfg!(feature = "cuda"));
        crate::models::ensure_whisper_model(&config.model_path)?;
        crate::models::ensure_vad_model(&config.vad_model_path)?;
        let model_path = config.model_path.to_str().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "model path is not valid UTF-8")
        })?;
        let model_path = CString::new(model_path)?;
        let vad_model_path = config.vad_model_path.to_str().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "VAD model path is not valid UTF-8",
            )
        })?;
        let vad_model_path = CString::new(vad_model_path)?;
        let language = config.language.map(CString::new).transpose()?;

        let mut context_params = unsafe { whisper::whisper_context_default_params() };
        context_params.use_gpu = cuda.use_cuda;
        context_params.flash_attn = true;
        let context = NonNull::new(unsafe {
            whisper::whisper_init_from_file_with_params(model_path.as_ptr(), context_params)
        })
        .ok_or_else(|| io::Error::other("failed to load Whisper model"))?;

        Ok((
            Self {
                context,
                vad_model_path,
                language,
            },
            cuda,
        ))
    }

    pub fn transcribe_wav(&mut self, path: impl AsRef<Path>) -> Result<Vec<Segment>> {
        let samples = read_pcm_16k_wav(path)?;
        self.transcribe(&samples)
    }

    pub fn transcribe(&mut self, samples: &[f32]) -> Result<Vec<Segment>> {
        let sample_count = validate_samples(samples)?;
        let strategy = whisper::whisper_sampling_strategy_WHISPER_SAMPLING_BEAM_SEARCH;
        let mut params = unsafe { whisper::whisper_full_default_params(strategy) };
        params.strategy = strategy;
        params.beam_search.beam_size = 5;
        params.beam_search.patience = -1.0;
        params.language = self
            .language
            .as_ref()
            .map_or(std::ptr::null(), |language| language.as_ptr());
        params.no_context = true;
        params.print_progress = false;
        params.print_realtime = false;
        params.print_timestamps = false;
        params.token_timestamps = true;
        params.max_len = 0;
        params.split_on_word = false;
        params.vad = true;
        params.vad_model_path = self.vad_model_path.as_ptr();
        params.vad_params = unsafe { whisper::whisper_vad_default_params() };
        params.vad_params.threshold = 0.5;
        params.vad_params.min_speech_duration_ms = 250;
        params.vad_params.min_silence_duration_ms = 100;
        params.vad_params.max_speech_duration_s = f32::MAX;
        params.vad_params.speech_pad_ms = 30;
        params.vad_params.samples_overlap = 0.1;

        let context = self.context.as_ptr();
        let result =
            unsafe { whisper::whisper_full(context, params, samples.as_ptr(), sample_count) };
        if result != 0 {
            return Err(
                io::Error::other(format!("Whisper transcription failed ({result})")).into(),
            );
        }

        let mut segments = Vec::new();
        for index in 0..unsafe { whisper::whisper_full_n_segments(context) } {
            let text = unsafe {
                let text = whisper::whisper_full_get_segment_text(context, index);
                if text.is_null() {
                    return Err(io::Error::other("Whisper returned a null segment").into());
                }
                CStr::from_ptr(text).to_str()?.to_owned()
            };
            if text.trim().is_empty() {
                continue;
            }
            segments.push(Segment {
                index: segments.len() + 1,
                start_ms: centiseconds_to_ms(unsafe {
                    whisper::whisper_full_get_segment_t0(context, index)
                }),
                end_ms: centiseconds_to_ms(unsafe {
                    whisper::whisper_full_get_segment_t1(context, index)
                }),
                text,
            });
        }
        Ok(segments)
    }
}

#[cfg(test)]
mod tests {
    use super::{centiseconds_to_ms, read_pcm_16k_wav_reader, resolve_cuda, validate_samples};
    use std::io::Cursor;

    #[test]
    fn falls_back_to_cpu_when_cuda_was_not_compiled() {
        assert_eq!(
            resolve_cuda(true, false),
            super::CudaResolution {
                use_cuda: false,
                warning: Some("CUDA was requested, but this build has no CUDA support; using CPU."),
            }
        );
    }

    #[test]
    fn keeps_cuda_enabled_when_compiled_and_requested() {
        assert_eq!(
            resolve_cuda(true, true),
            super::CudaResolution {
                use_cuda: true,
                warning: None,
            }
        );
    }

    #[test]
    fn converts_whisper_centiseconds_to_milliseconds() {
        assert_eq!(centiseconds_to_ms(512), 5_120);
    }

    #[test]
    fn rejects_empty_or_non_finite_samples() {
        assert!(validate_samples(&[]).is_err());
        assert!(validate_samples(&[0.0, f32::NAN]).is_err());
        assert_eq!(validate_samples(&[0.0, 0.5]).unwrap(), 2);
    }

    #[test]
    fn decodes_the_same_mono_pcm_format_used_by_ffmpeg() {
        let mut cursor = Cursor::new(Vec::new());
        {
            let spec = hound::WavSpec {
                channels: 1,
                sample_rate: 16_000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            };
            let mut writer = hound::WavWriter::new(&mut cursor, spec).unwrap();
            writer.write_sample(-16_384_i16).unwrap();
            writer.write_sample(16_384_i16).unwrap();
            writer.finalize().unwrap();
        }

        let reader = hound::WavReader::new(Cursor::new(cursor.into_inner())).unwrap();
        let samples = read_pcm_16k_wav_reader(reader).unwrap();
        assert_eq!(samples, [-0.5, 0.5]);
    }
}
