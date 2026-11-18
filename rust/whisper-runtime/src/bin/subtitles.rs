use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;
use whisper_runtime::config::AppConfig;
use whisper_runtime::pipeline::{run_pipeline, PipelineConfig, PipelineOptions};
use whisper_runtime::server::serve;

#[derive(Debug, Parser)]
#[command(name = "subtitles", version, about = "Generate English subtitles from speech")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Generate(GenerateArgs),
    Serve,
}

#[derive(Debug, Args)]
struct GenerateArgs {
    video: PathBuf,
    #[arg(short = 'l', long, default_value = "auto", value_parser = ["ja", "zh", "auto"])]
    language: String,
    #[arg(long)]
    skip_refine: bool,
    #[arg(short, long)]
    output: Option<PathBuf>,
    #[arg(long)]
    ollama_model: Option<String>,
    #[arg(long)]
    whisper_model: Option<String>,
    #[arg(long)]
    keep_audio: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Generate(args) => {
            let app_config = AppConfig::from_env()?;
            let options = PipelineOptions {
                language: args.language,
                skip_refine: args.skip_refine,
                output_path: args.output,
                ollama_model: args.ollama_model.unwrap_or(app_config.ollama_model.clone()),
                whisper_model: args.whisper_model.unwrap_or(app_config.whisper_model.clone()),
                keep_audio: args.keep_audio,
            };
            let config = PipelineConfig {
                deepl_api_key: app_config.deepl_api_key,
                ollama_host: app_config.ollama_host,
                model_root_path: app_config.model_root_path,
            };
            let output = run_pipeline(args.video, &options, &config)?;
            println!("SRT generated: {}", output.display());
        }
        Commands::Serve => serve(AppConfig::from_env()?).await?,
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::Cli;
    use clap::Parser;

    #[test]
    fn parses_generate_flags_and_defaults() {
        let cli = Cli::try_parse_from(["subtitles", "generate", "clip.mp4", "-l", "ja", "--skip-refine", "-o", "custom.srt", "--keep-audio"]).unwrap();
        match cli.command {
            super::Commands::Generate(args) => {
                assert_eq!(args.video.to_string_lossy(), "clip.mp4");
                assert_eq!(args.language, "ja");
                assert!(args.skip_refine);
                assert_eq!(args.output.unwrap().to_string_lossy(), "custom.srt");
                assert!(args.keep_audio);
            }
            super::Commands::Serve => panic!("expected generate command"),
        }
    }

    #[test]
    fn rejects_unsupported_languages() {
        assert!(Cli::try_parse_from(["subtitles", "generate", "clip.mp4", "--language", "fr"]).is_err());
    }
}