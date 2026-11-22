use crate::audio::{extract_audio, split_wav};
use crate::models::{vad_model_path, whisper_model_path};
use crate::segments::{collapse_repeats, generate_srt};
use crate::services::{refine_segments, translate_segments};
use crate::{Result, WhisperConfig, WhisperRuntime};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const VAD_MODEL_NAME: &str = "silero-v6.2.0";
const AUDIO_CHUNK_SECONDS: u32 = 600;

#[derive(Clone, Debug)]
pub struct PipelineOptions {
    pub language: String,
    pub skip_refine: bool,
    pub output_path: Option<PathBuf>,
    pub ollama_model: String,
    pub whisper_model: String,
    pub keep_audio: bool,
}

#[derive(Clone, Debug)]
pub struct PipelineConfig {
    pub deepl_api_key: String,
    pub ollama_host: String,
    pub model_root_path: PathBuf,
}

pub fn default_output_path(input_path: &Path) -> PathBuf {
    let stem = input_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("subtitle");
    input_path.with_file_name(format!("{stem}.en.srt"))
}

fn validate_options(options: &PipelineOptions) -> Result<()> {
    if !matches!(options.language.as_str(), "ja" | "zh" | "pt" | "de" | "es" | "ru" | "auto") {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "language must be ja, zh, pt, de, es, ru, or auto").into());
    }


    Ok(())
}

fn renumber(segments: &mut [crate::Segment]) {
    for (index, segment) in segments.iter_mut().enumerate() {
        segment.index = index + 1;
    }
}

pub fn run_pipeline(
    video_path: impl AsRef<Path>,
    options: &PipelineOptions,
    config: &PipelineConfig,
) -> Result<PathBuf> {
    validate_options(options)?;
    let video_path = video_path.as_ref();
    let output_path = options
        .output_path
        .clone()
        .unwrap_or_else(|| default_output_path(video_path));
    let model_path = whisper_model_path(&config.model_root_path, &options.whisper_model)?;
    let vad_path = vad_model_path(&config.model_root_path, VAD_MODEL_NAME)?;
    let (mut runtime, cuda) = WhisperRuntime::new(WhisperConfig {
        model_path,
        vad_model_path: vad_path,
        language: (options.language != "auto").then(|| options.language.clone()),
        request_cuda: true,
    })?;
    if let Some(warning) = cuda.warning {
        eprintln!("{warning}");
    }

    let mut wav = extract_audio(video_path)?;
    if options.keep_audio {
        wav.preserve();
    }
    let chunks = split_wav(&wav.path, AUDIO_CHUNK_SECONDS)?;
    let mut segments = Vec::new();
    for (index, chunk) in chunks.chunks.iter().enumerate() {
        eprintln!("Pedaço {}/{}", index + 1, chunks.chunks.len());
        let chunk_segments = runtime.transcribe_wav(&chunk.path)?;
        segments.extend(chunk_segments.into_iter().map(|mut segment| {
            segment.start_ms += chunk.start_ms;
            segment.end_ms += chunk.start_ms;
            segment
        }));
    }
    drop(chunks);

    renumber(&mut segments);
    segments = collapse_repeats(&segments);
    if !options.skip_refine {
        segments = refine_segments(
            &segments,
            &options.language,
            &options.ollama_model,
            &config.ollama_host,
        );
    }
    segments = translate_segments(&segments, &options.language, &config.deepl_api_key)
        .map_err(io::Error::other)?;
    segments.retain(|segment| !segment.text.trim().is_empty());
    renumber(&mut segments);

    if let Some(parent) = output_path.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output_path, generate_srt(&segments))?;
    Ok(output_path)
}

#[cfg(test)]
mod tests {
    use super::{default_output_path, validate_options, PipelineOptions};
    use std::path::Path;

    fn options(language: &str) -> PipelineOptions {
        PipelineOptions {
            language: language.to_owned(),
            skip_refine: false,
            output_path: None,
            ollama_model: "test-model".into(),
            whisper_model: "large-v3".into(),
            keep_audio: false,
        }
    }

    #[test]
    fn derives_english_srt_path_from_input() {
        assert_eq!(default_output_path(Path::new("/video/my clip.mp4")), Path::new("/video/my clip.en.srt"));
        assert_eq!(default_output_path(Path::new("clip.wav")), Path::new("clip.en.srt"));
    }

    #[test]
    fn validates_supported_source_languages() {
        for language in ["ja", "zh", "en", "es", "pt", "de", "ru", "auto"] {
            assert!(validate_options(&options(language)).is_ok());
        }
        assert!(validate_options(&options("fr")).is_err());
    }
}