import type { Segment } from '../types/subtitles.js'

export type WhisperCallOptions = {
  modelName: string
  modelRootPath: string
  withCuda: boolean
  removeWavFileAfterTranscription: false
  whisperOptions: { language?: string; outputInJsonFull: true }
}

type DetailedSegment = { start: number; end: number; text: string }
type DetailedResult = { segments: DetailedSegment[] }
type Transcriber = (audioPath: string, options: WhisperCallOptions) => Promise<DetailedResult>

export function resolveCuda({ requested, cudaAddonAvailable }: { requested: boolean; cudaAddonAvailable: boolean }) {
  if (requested && !cudaAddonAvailable) {
    return { withCuda: false, warning: 'CUDA was requested, but its addon is unavailable; using CPU.' }
  }
  return { withCuda: requested }
}

export function buildWhisperOptions({
  language, modelName, modelRootPath, withCuda,
}: { language: string; modelName: string; modelRootPath: string; withCuda: boolean }): WhisperCallOptions {
  return {
    modelName, modelRootPath, withCuda, removeWavFileAfterTranscription: false,
    whisperOptions: { ...(language === 'auto' ? {} : { language }), outputInJsonFull: true },
  }
}

const defaultTranscriber: Transcriber = async (audioPath, options) => {
  const packageName = 'nodejs-whisper'
  const whisper = await import(packageName) as { transcribeDetailed: Transcriber }
  return whisper.transcribeDetailed(audioPath, options)
}

export async function transcribe(
  audioPath: string,
  options: WhisperCallOptions,
  transcriber: Transcriber = defaultTranscriber,
): Promise<Segment[]> {
  const { segments } = await transcriber(audioPath, options)
  return segments.filter(({ text }) => text.trim() !== '').map(({ start, end, text }, index) => ({
    index: index + 1, startMs: start, endMs: end, text,
  }))
}
