use crate::config::AppConfig;
use crate::pipeline::{run_pipeline, PipelineConfig, PipelineOptions};
use axum::body::Body;
use axum::extract::{DefaultBodyLimit, Multipart, State};
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_TYPE, COOKIE, SET_COOKIE};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use percent_encoding::{utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tower_http::limit::RequestBodyLimitLayer;
use uuid::Uuid;

const CSRF_COOKIE: &str = "subtitle_csrf";
const SUPPORTED_EXTENSIONS: &[&str] = &[
    "mp4", "mkv", "mov", "webm", "avi", "mp3", "wav", "m4a", "flac", "ogg",
];
const RFC5987_ENCODE_SET: &AsciiSet = &NON_ALPHANUMERIC.remove(b'.').remove(b'-').remove(b'_');
const HOME_PAGE: &str = include_str!("../resources/home.html");

#[derive(Clone)]
struct AppState {
    config: AppConfig,
    processing: Arc<AtomicBool>,
}

struct ProcessingGuard(Arc<AtomicBool>);

impl Drop for ProcessingGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

#[derive(Default)]
struct TemporaryFiles(Vec<PathBuf>);

impl Drop for TemporaryFiles {
    fn drop(&mut self) {
        for path in &self.0 {
            let _ = fs::remove_file(path);
        }
    }
}

struct Upload {
    path: PathBuf,
    client_name: String,
}

pub async fn serve(config: AppConfig) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let body_limit = config
        .max_upload_mb
        .saturating_mul(1024 * 1024)
        .saturating_add(1024 * 1024);
    let state = AppState {
        config: config.clone(),
        processing: Arc::new(AtomicBool::new(false)),
    };
    let app = Router::new()
        .route("/", get(home))
        .route("/subtitles", post(subtitles))
        .with_state(state)
        .layer(DefaultBodyLimit::disable())
        .layer(RequestBodyLimitLayer::new(body_limit));
    let listener = tokio::net::TcpListener::bind((config.host.as_str(), config.port)).await?;
    eprintln!("Subtitle server listening on http://{}:{}", config.host, config.port);
    axum::serve(listener, app).await?;
    Ok(())
}

async fn home() -> Response {
    let token = Uuid::new_v4().to_string();
    let page = HOME_PAGE.replace("__CSRF_TOKEN__", &token);
    let mut response = (StatusCode::OK, [(CONTENT_TYPE, "text/html; charset=utf-8")], page).into_response();
    if let Ok(cookie) = HeaderValue::from_str(&format!(
        "{CSRF_COOKIE}={token}; Path=/; SameSite=Strict; HttpOnly"
    )) {
        response.headers_mut().insert(SET_COOKIE, cookie);
    }
    response
}

fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .filter_map(|cookie| cookie.trim().split_once('='))
        .find_map(|(key, value)| (key == name).then(|| value.to_owned()))
}

fn supported_extension(filename: &str) -> Option<String> {
    let filename = filename.replace('\\', "/");
    let extension = Path::new(filename.rsplit('/').next()?)
        .extension()?
        .to_str()?
        .to_ascii_lowercase();
    SUPPORTED_EXTENSIONS
        .contains(&extension.as_str())
        .then_some(extension)
}

fn download_stem(filename: &str) -> String {
    let filename = filename.replace('\\', "/");
    let base = filename.rsplit('/').next().unwrap_or_default();
    Path::new(base)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("subtitle")
        .chars()
        .filter(|character| !character.is_control() && *character != '/' && *character != '\\')
        .collect::<String>()
        .trim()
        .to_owned()
}

fn bad_request(message: &'static str) -> Response {
    (StatusCode::BAD_REQUEST, message).into_response()
}

async fn subtitles(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Response {
    if state
        .processing
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return (StatusCode::SERVICE_UNAVAILABLE, "A subtitle is already being processed. Please try again shortly.").into_response();
    }
    let _processing = ProcessingGuard(state.processing.clone());
    let max_upload_bytes = state.config.max_upload_mb.saturating_mul(1024 * 1024) as u64;
    let mut temporary_files = TemporaryFiles::default();
    let mut upload: Option<Upload> = None;
    let mut csrf = None;
    let mut language = "auto".to_owned();
    let mut skip_refine = false;

    loop {
        let field = match multipart.next_field().await {
            Ok(Some(field)) => field,
            Ok(None) => break,
            Err(_) => return bad_request("Invalid multipart request."),
        };
        let name = field.name().unwrap_or_default().to_owned();
        match name.as_str() {
            "video" => {
                if upload.is_some() {
                    return bad_request("Only one video file is allowed.");
                }
                let Some(client_name) = field.file_name().map(str::to_owned) else {
                    return bad_request("The video field is required.");
                };
                let Some(extension) = supported_extension(&client_name) else {
                    return bad_request("Unsupported file extension.");
                };
                let directory = PathBuf::from("tmp/uploads");
                if tokio::fs::create_dir_all(&directory).await.is_err() {
                    return (StatusCode::INTERNAL_SERVER_ERROR, "Could not store upload.").into_response();
                }
                let path = directory.join(format!("{}.{}", Uuid::new_v4(), extension.to_ascii_lowercase()));
                let mut file = match tokio::fs::File::create(&path).await {
                    Ok(file) => file,
                    Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Could not store upload.").into_response(),
                };
                let mut size = 0_u64;
                let mut field = field;
                loop {
                    let chunk = match field.chunk().await {
                        Ok(Some(chunk)) => chunk,
                        Ok(None) => break,
                        Err(_) => return bad_request("Invalid multipart file data."),
                    };
                    size = size.saturating_add(chunk.len() as u64);
                    if size > max_upload_bytes {
                        return (StatusCode::PAYLOAD_TOO_LARGE, "Uploaded file exceeds the configured size limit.").into_response();
                    }
                    if file.write_all(&chunk).await.is_err() {
                        return (StatusCode::INTERNAL_SERVER_ERROR, "Could not store upload.").into_response();
                    }
                }
                if file.flush().await.is_err() {
                    return (StatusCode::INTERNAL_SERVER_ERROR, "Could not store upload.").into_response();
                }
                drop(file);
                temporary_files.0.push(path.clone());
                upload = Some(Upload { path, client_name });
            }
            "_csrf" => match field.text().await {
                Ok(value) => csrf = Some(value),
                Err(_) => return bad_request("Invalid CSRF token."),
            },
            "language" => match field.text().await {
                Ok(value) => language = value,
                Err(_) => return bad_request("Invalid language."),
            },
            "skipRefine" => match field.text().await {
                Ok(value) if ["", "true", "false", "1", "0", "on", "off"].contains(&value.as_str()) => {
                    skip_refine = matches!(value.as_str(), "true" | "1" | "on");
                }
                _ => return bad_request("Invalid skipRefine value."),
            },
            _ => {
                if field.bytes().await.is_err() {
                    return bad_request("Invalid multipart field.");
                }
            }
        }
    }

    let Some(cookie) = cookie_value(&headers, CSRF_COOKIE) else {
        return bad_request("Invalid CSRF token.");
    };
    if csrf.as_deref() != Some(cookie.as_str()) {
        return bad_request("Invalid CSRF token.");
    }
    if !matches!(language.as_str(), "ja" | "zh" | "auto") {
        return bad_request("Invalid language.");
    }
    let Some(upload) = upload else {
        return bad_request("The video field is required.");
    };

    let output_path = PathBuf::from("tmp/uploads").join(format!("{}.srt", Uuid::new_v4()));
    temporary_files.0.push(output_path.clone());
    let options = PipelineOptions {
        language,
        skip_refine,
        output_path: Some(output_path.clone()),
        ollama_model: state.config.ollama_model.clone(),
        whisper_model: state.config.whisper_model.clone(),
        keep_audio: false,
    };
    let pipeline_config = PipelineConfig {
        deepl_api_key: state.config.deepl_api_key.clone(),
        ollama_host: state.config.ollama_host.clone(),
        model_root_path: state.config.model_root_path.clone(),
    };
    let input_path = upload.path.clone();
    let generated = match tokio::task::spawn_blocking(move || {
        run_pipeline(input_path, &options, &pipeline_config)
    })
    .await
    {
        Ok(Ok(path)) => path,
        Ok(Err(error)) => {
            eprintln!("Subtitle generation failed: {error}");
            return (StatusCode::INTERNAL_SERVER_ERROR, "Subtitle processing failed. Please try again.").into_response();
        }
        Err(error) => {
            eprintln!("Subtitle worker failed: {error}");
            return (StatusCode::INTERNAL_SERVER_ERROR, "Subtitle processing failed. Please try again.").into_response();
        }
    };
    let body = match tokio::fs::read(generated).await {
        Ok(body) => body,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Subtitle processing failed. Please try again.").into_response(),
    };
    let stem = download_stem(&upload.client_name);
    let filename = format!("{stem}.en.srt");
    let encoded = utf8_percent_encode(&filename, RFC5987_ENCODE_SET);
    let mut response = Response::new(Body::from(body));
    *response.status_mut() = StatusCode::OK;
    response.headers_mut().insert(CONTENT_TYPE, HeaderValue::from_static("application/x-subrip; charset=utf-8"));
    if let Ok(value) = HeaderValue::from_str(&format!(
        "attachment; filename=\"legenda.en.srt\"; filename*=UTF-8''{encoded}"
    )) {
        response.headers_mut().insert(CONTENT_DISPOSITION, value);
    }
    response
}

#[cfg(test)]
mod tests {
    use super::{cookie_value, download_stem, supported_extension, CSRF_COOKIE};
    use axum::http::{header::COOKIE, HeaderMap, HeaderValue};

    #[test]
    fn validates_upload_extensions_case_insensitively() {
        assert_eq!(supported_extension("clip.MP4").as_deref(), Some("mp4"));
        assert_eq!(supported_extension("clip.exe"), None);
    }

    #[test]
    fn sanitizes_download_name_without_losing_unicode() {
        assert_eq!(download_stem("../動画 clip.mp4"), "動画 clip");
        assert_eq!(download_stem("C:\\folder\\clip.mov"), "clip");
    }

    #[test]
    fn reads_csrf_cookie_from_cookie_header() {
        let mut headers = HeaderMap::new();
        headers.insert(COOKIE, HeaderValue::from_static("other=value; subtitle_csrf=token"));
        assert_eq!(cookie_value(&headers, CSRF_COOKIE).as_deref(), Some("token"));
    }
}