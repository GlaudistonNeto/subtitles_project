use std::env;
use std::path::PathBuf;
use whisper_runtime::{WhisperConfig, WhisperRuntime};

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut args = env::args().skip(1);
    let model_path = args
        .next()
        .ok_or("usage: whisper_probe MODEL VAD WAV [LANGUAGE]")?;
    let vad_model_path = args
        .next()
        .ok_or("usage: whisper_probe MODEL VAD WAV [LANGUAGE]")?;
    let audio_path = args
        .next()
        .ok_or("usage: whisper_probe MODEL VAD WAV [LANGUAGE]")?;
    let language = args.next().filter(|value| value != "auto");
    let (mut runtime, cuda) = WhisperRuntime::new(WhisperConfig {
        model_path: PathBuf::from(model_path),
        vad_model_path: PathBuf::from(vad_model_path),
        language,
        request_cuda: true,
    })?;
    if let Some(warning) = cuda.warning {
        eprintln!("{warning}");
    }
    for segment in runtime.transcribe_wav(audio_path)? {
        println!(
            "{}\t{}\t{}\t{}",
            segment.index, segment.start_ms, segment.end_ms, segment.text
        );
    }
    Ok(())
}
