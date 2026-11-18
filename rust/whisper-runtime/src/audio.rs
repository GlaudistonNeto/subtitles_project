use crate::Result;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use uuid::Uuid;

fn run_process(program: &str, args: &[String]) -> Result<Output> {
    let output = Command::new(program).args(args).output()?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        return Err(io::Error::other(format!(
            "{program} failed with status {}: {detail}",
            output.status
        ))
        .into());
    }
    Ok(output)
}

pub struct TemporaryWav {
    pub path: PathBuf,
    keep: bool,
}

impl TemporaryWav {
    pub fn preserve(&mut self) {
        self.keep = true;
    }
}

impl Drop for TemporaryWav {
    fn drop(&mut self) {
        if !self.keep {
            let _ = fs::remove_file(&self.path);
        }
    }
}

pub fn extract_audio(video_path: &Path) -> Result<TemporaryWav> {
    let temporary_directory = PathBuf::from("tmp");
    fs::create_dir_all(&temporary_directory)?;
    let wav_path = temporary_directory.join(format!("{}.wav", Uuid::new_v4()));
    let mut wav = TemporaryWav {
        path: wav_path.clone(),
        keep: false,
    };
    let args = vec![
        "-nostdin".into(),
        "-loglevel".into(),
        "error".into(),
        "-i".into(),
        video_path.to_string_lossy().into_owned(),
        "-vn".into(),
        "-af".into(),
        "highpass=f=80,loudnorm".into(),
        "-ar".into(),
        "16000".into(),
        "-ac".into(),
        "1".into(),
        "-c:a".into(),
        "pcm_s16le".into(),
        wav_path.to_string_lossy().into_owned(),
    ];
    if let Err(error) = run_process("ffmpeg", &args) {
        wav.keep = false;
        return Err(error);
    }
    Ok(wav)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioChunk {
    pub path: PathBuf,
    pub start_ms: i64,
    pub duration_ms: i64,
}

pub struct ChunkSet {
    pub chunks: Vec<AudioChunk>,
    directory: PathBuf,
}

impl Drop for ChunkSet {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

pub fn compute_offsets(durations_ms: &[i64]) -> Vec<i64> {
    let mut offset = 0;
    durations_ms
        .iter()
        .map(|duration| {
            let start = offset;
            offset += duration;
            start
        })
        .collect()
}

fn is_chunk_file(name: &str) -> bool {
    name.strip_prefix("chunk_")
        .and_then(|name| name.strip_suffix(".wav"))
        .is_some_and(|number| !number.is_empty() && number.bytes().all(|byte| byte.is_ascii_digit()))
}

pub fn split_wav(wav_path: &Path, chunk_seconds: u32) -> Result<ChunkSet> {
    let directory = PathBuf::from("tmp").join(Uuid::new_v4().to_string());
    fs::create_dir_all(&directory)?;
    let mut guard = ChunkSet {
        chunks: Vec::new(),
        directory: directory.clone(),
    };

    let pattern = directory.join("chunk_%04d.wav");
    let args = vec![
        "-nostdin".into(),
        "-loglevel".into(),
        "error".into(),
        "-i".into(),
        wav_path.to_string_lossy().into_owned(),
        "-f".into(),
        "segment".into(),
        "-segment_time".into(),
        chunk_seconds.to_string(),
        "-c".into(),
        "copy".into(),
        "-reset_timestamps".into(),
        "1".into(),
        pattern.to_string_lossy().into_owned(),
    ];
    run_process("ffmpeg", &args)?;

    let mut files: Vec<_> = fs::read_dir(&directory)?
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| is_chunk_file(name))
        .collect();
    files.sort();

    let mut durations = Vec::with_capacity(files.len());
    let mut paths = Vec::with_capacity(files.len());
    for file in files {
        let path = directory.join(file);
        let args = vec![
            "-v".into(),
            "error".into(),
            "-show_entries".into(),
            "format=duration".into(),
            "-of".into(),
            "csv=p=0".into(),
            path.to_string_lossy().into_owned(),
        ];
        let output = run_process("ffprobe", &args)?;
        let duration = String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse::<f64>()?;
        if !duration.is_finite() || duration < 0.0 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "ffprobe returned an invalid duration").into());
        }
        durations.push((duration * 1_000.0).round() as i64);
        paths.push(path);
    }

    let offsets = compute_offsets(&durations);
    guard.chunks = paths
        .into_iter()
        .zip(durations)
        .zip(offsets)
        .map(|((path, duration_ms), start_ms)| AudioChunk {
            path,
            start_ms,
            duration_ms,
        })
        .collect();
    Ok(guard)
}

#[cfg(test)]
mod tests {
    use super::{compute_offsets, is_chunk_file};

    #[test]
    fn computes_offsets_from_measured_chunk_durations() {
        assert_eq!(compute_offsets(&[30_080, 29_952, 30_080]), [0, 30_080, 60_032]);
    }

    #[test]
    fn accepts_only_numbered_chunk_wav_names() {
        assert!(is_chunk_file("chunk_0000.wav"));
        assert!(is_chunk_file("chunk_42.wav"));
        assert!(!is_chunk_file("chunk_.wav"));
        assert!(!is_chunk_file("other_0000.wav"));
        assert!(!is_chunk_file("chunk_0000.mp3"));
    }
}