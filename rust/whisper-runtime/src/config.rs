use crate::Result;
use std::env;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub deepl_api_key: String,
    pub ollama_host: String,
    pub ollama_model: String,
    pub whisper_model: String,
    pub model_root_path: PathBuf,
    pub host: String,
    pub port: u16,
    pub max_upload_mb: usize,
}

impl AppConfig {
    pub fn from_env() -> Result<Self> {
        let _ = dotenvy::dotenv();
        let port = env::var("PORT").unwrap_or_else(|_| "3333".into()).parse()?;
        let max_upload_mb = env::var("MAX_UPLOAD_MB")
            .unwrap_or_else(|_| "500".into())
            .parse()?;
        Ok(Self {
            deepl_api_key: env::var("DEEPL_API_KEY").unwrap_or_default(),
            ollama_host: env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://localhost:11434".into()),
            ollama_model: env::var("OLLAMA_MODEL").unwrap_or_else(|_| "qwen2.5:14b".into()),
            whisper_model: env::var("WHISPER_MODEL").unwrap_or_else(|_| "large-v3".into()),
            model_root_path: env::var_os("WHISPER_MODEL_ROOT")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("tmp/whisper-models")),
            host: env::var("HOST").unwrap_or_else(|_| "localhost".into()),
            port,
            max_upload_mb,
        })
    }
}