use crate::Result;
use reqwest::blocking::Client;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;
use uuid::Uuid;

const GGML_MAGIC: [u8; 4] = *b"lmgg";
const MODEL_NAMES: &[&str] = &[
    "tiny", "tiny.en", "tiny-q5_1", "tiny.en-q5_1", "tiny-q8_0", "base", "base.en",
    "base-q5_1", "base.en-q5_1", "base-q8_0", "small", "small.en", "small.en-tdrz",
    "small-q5_1", "small.en-q5_1", "small-q8_0", "medium", "medium.en", "medium-q5_0",
    "medium.en-q5_0", "medium-q8_0", "large-v1", "large-v2", "large-v2-q5_0",
    "large-v2-q8_0", "large-v3", "large-v3-q5_0", "large-v3-turbo",
    "large-v3-turbo-q5_0", "large-v3-turbo-q8_0",
];
const VAD_NAMES: &[&str] = &["silero-v5.1.2", "silero-v6.2.0"];

pub fn whisper_model_path(root: &Path, name: &str) -> Result<PathBuf> {
    let canonical = if name == "large" { "large-v3" } else { name };
    if !MODEL_NAMES.contains(&canonical) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "unsupported Whisper model name").into());
    }
    Ok(root.join(format!("ggml-{canonical}.bin")))
}

pub fn vad_model_path(root: &Path, name: &str) -> Result<PathBuf> {
    if !VAD_NAMES.contains(&name) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "unsupported VAD model name").into());
    }
    Ok(root.join(format!("ggml-{name}.bin")))
}

pub fn is_ggml_model(path: &Path) -> bool {
    let mut magic = [0; 4];
    File::open(path)
        .and_then(|mut file| file.read_exact(&mut magic))
        .is_ok()
        && magic == GGML_MAGIC
}

fn ensure_model(path: &Path, vad: bool) -> Result<()> {
    if is_ggml_model(path) {
        return Ok(());
    }
    if path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("existing model is invalid: {}", path.display()),
        )
        .into());
    }

    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid model filename"))?;
    let allowed = if vad {
        VAD_NAMES.iter().any(|name| filename == format!("ggml-{name}.bin"))
    } else {
        MODEL_NAMES.iter().any(|name| filename == format!("ggml-{name}.bin"))
    };
    if !allowed {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "unsupported model filename").into());
    }

    let repository = if vad {
        "https://huggingface.co/ggml-org/whisper-vad/resolve/main"
    } else {
        "https://huggingface.co/ggerganov/whisper.cpp/resolve/main"
    };
    download_model_file(&format!("{repository}/{filename}"), path)
}

pub fn ensure_whisper_model(path: &Path) -> Result<()> {
    ensure_model(path, false)
}

pub fn ensure_vad_model(path: &Path) -> Result<()> {
    ensure_model(path, true)
}

pub fn download_model_file(url: &str, destination: &Path) -> Result<()> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    let filename = destination
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid model destination"))?;
    let temporary_path = destination.with_file_name(format!("{filename}.{}.download", Uuid::new_v4()));
    let result = (|| -> Result<()> {
        let client = Client::builder().timeout(Duration::from_secs(60)).build()?;
        let mut response = client.get(url).send()?.error_for_status()?;
        let mut temporary = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)?;
        io::copy(&mut response, &mut temporary)?;
        temporary.flush()?;
        drop(temporary);
        if !is_ggml_model(&temporary_path) {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "downloaded file is not a GGML model").into());
        }
        fs::rename(&temporary_path, destination)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary_path);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::{is_ggml_model, vad_model_path, whisper_model_path, GGML_MAGIC};
    use std::fs;
    use std::io::Write;
    use uuid::Uuid;

    #[test]
    fn resolves_supported_model_files_and_large_alias() {
        let root = std::path::Path::new("tmp/models");
        assert_eq!(whisper_model_path(root, "large-v3").unwrap(), root.join("ggml-large-v3.bin"));
        assert_eq!(whisper_model_path(root, "large").unwrap(), root.join("ggml-large-v3.bin"));
        assert!(whisper_model_path(root, "../../secret").is_err());
        assert_eq!(vad_model_path(root, "silero-v6.2.0").unwrap(), root.join("ggml-silero-v6.2.0.bin"));
    }

    #[test]
    fn validates_the_ggml_magic() {
        let path = std::env::temp_dir().join(format!("{}.bin", Uuid::new_v4()));
        let mut file = fs::File::create(&path).unwrap();
        file.write_all(&GGML_MAGIC).unwrap();
        drop(file);
        assert!(is_ggml_model(&path));
        fs::write(&path, b"bad!").unwrap();
        assert!(!is_ggml_model(&path));
        fs::remove_file(path).unwrap();
    }
}