import { parse, join } from 'node:path'
import { collapseRepeats } from './segment_filters.js'
import { buildWhisperOptions, resolveCuda } from './transcription_service.js'
import type { WhisperCallOptions } from './transcription_service.js'
import type { Segment } from '../types/subtitles.js'

export type PipelineOptions = {
  language: 'ja' | 'zh' | 'auto'
  skipRefine: boolean
  outputPath?: string
  ollamaModel: string
  whisperModel: string
  keepAudio?: boolean
}

export type PipelineDeps = {
  audio: {
    withAudio<T>(videoPath: string, consumer: (wavPath: string) => Promise<T>, keepAudio?: boolean): Promise<T>
  }
  transcribe: (wavPath: string, options: WhisperCallOptions) => Promise<Segment[]>
  refine: (segments: Segment[], language: string, model?: string) => Promise<Segment[]>
  translate: (segments: Segment[], language: string, apiKey: string) => Promise<Segment[]>
  generateSrt: (segments: Segment[]) => string
  writeFile: (path: string, content: string) => Promise<void>
  detectCudaAddon: () => boolean
  resolveCuda: typeof resolveCuda
  deeplApiKey: string
  modelRootPath: string
  logger: { warn(message: string): void }
}

export async function runPipeline(
  videoPath: string, options: PipelineOptions, deps: PipelineDeps,
): Promise<string> {
  const parsedPath = parse(videoPath)
  const outputPath = options.outputPath ?? join(parsedPath.dir, `${parsedPath.name}.en.srt`)
  const cuda = deps.resolveCuda({ requested: true, cudaAddonAvailable: deps.detectCudaAddon() })
  if (!cuda.withCuda && 'warning' in cuda && cuda.warning) deps.logger.warn(cuda.warning)
  const whisperOptions = buildWhisperOptions({
    language: options.language, modelName: options.whisperModel,
    modelRootPath: deps.modelRootPath, withCuda: cuda.withCuda,
  })
  return deps.audio.withAudio(videoPath, async (wavPath) => {
    let segments = collapseRepeats(await deps.transcribe(wavPath, whisperOptions))
    if (!options.skipRefine) segments = await deps.refine(segments, options.language, options.ollamaModel)
    segments = await deps.translate(segments, options.language, deps.deeplApiKey)
    segments = segments.filter((segment) => segment.text.trim().length > 0)
      .map((segment, index) => ({ ...segment, index: index + 1 }))
    await deps.writeFile(outputPath, deps.generateSrt(segments))
    return outputPath
  }, options.keepAudio ?? false)
}
