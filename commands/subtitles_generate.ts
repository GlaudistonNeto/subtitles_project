import { BaseCommand, args, flags } from '@adonisjs/core/ace'
import type { CommandOptions } from '@adonisjs/core/types/ace'
import env from '#start/env'
import { runPipeline, type PipelineDeps, type PipelineOptions } from '#services/subtitle_pipeline'
import { buildPipelineDeps } from '#services/pipeline_deps'

export default class SubtitlesGenerateCommand extends BaseCommand {
  static commandName = 'subtitles:generate'
  static description = 'Generate an English SRT subtitle file from a video'
  static options: CommandOptions = { startApp: true }

  @args.string({ description: 'Video file to transcribe' })
  declare video: string

  @flags.string({ alias: ['l'], default: 'auto', description: 'Source language: ja, zh, or auto' })
  declare language: string

  @flags.boolean({ description: 'Skip Ollama transcription refinement' })
  declare skipRefine?: boolean

  @flags.string({ alias: ['o'], description: 'Output SRT path' })
  declare output?: string

  @flags.string({ description: 'Ollama model to use for refinement' })
  declare ollamaModel?: string

  @flags.string({ description: 'Whisper model to use for transcription' })
  declare whisperModel?: string

  @flags.boolean({ description: 'Keep the temporary WAV file' })
  declare keepAudio?: boolean

  protected runPipeline(video: string, options: PipelineOptions, deps: PipelineDeps): Promise<string> {
    return runPipeline(video, options, deps)
  }

  async run() {
    const language = this.language
    if (language !== 'ja' && language !== 'zh' && language !== 'auto') {
      this.logger.error('Invalid --language value. Use ja, zh, or auto.')
      this.exitCode = 1
      return
    }
    const options: PipelineOptions = {
      language, skipRefine: this.skipRefine ?? false, outputPath: this.output,
      ollamaModel: this.ollamaModel ?? env.get('OLLAMA_MODEL'),
      whisperModel: this.whisperModel ?? env.get('WHISPER_MODEL'), keepAudio: this.keepAudio ?? false,
    }
    try {
      this.logger.info(`SRT generated: ${await this.runPipeline(this.video, options, buildPipelineDeps((message) => this.logger.info(message)))}`)
    } catch (error) {
      const detail = error instanceof Error ? error.message : 'unknown error'
      this.logger.error(`Subtitle generation failed: ${detail}`)
      this.exitCode = 1
    }
  }
}