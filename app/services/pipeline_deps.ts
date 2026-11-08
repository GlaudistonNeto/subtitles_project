import { writeFile } from 'node:fs/promises'
import { join } from 'node:path'
import env from '#start/env'
import { AudioService } from '#services/audio_service'
import { splitWav } from '#services/audio_chunker'
import { refineSegments } from '#services/refinement_service'
import { generateSrt } from '#services/srt_service'
import { detectCudaAddon, resolveCuda, transcribe } from '#services/transcription_service'
import { translateSegments } from '#services/translation_service'
import type { PipelineDeps } from '#services/subtitle_pipeline'

export function buildPipelineDeps(logWarning: (message: string) => void): PipelineDeps {
  return {
    audio: new AudioService(), splitWav, transcribe, refine: (segments, language, model) =>
      refineSegments(segments, language, model, env.get('OLLAMA_HOST')),
    translate: translateSegments, generateSrt, writeFile: (path, content) => writeFile(path, content, 'utf8'),
    detectCudaAddon, resolveCuda, deeplApiKey: env.get('DEEPL_API_KEY'),
    modelRootPath: join(process.cwd(), 'tmp', 'whisper-models'),
    logger: { warn: logWarning, info: logWarning },
  }
}