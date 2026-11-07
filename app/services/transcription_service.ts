import { existsSync, readFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import { dirname, join } from 'node:path'
import type { Segment } from '../types/subtitles.js'

const require = createRequire(import.meta.url)

export type WhisperCallOptions = {
  modelName: string
  modelRootPath: string
  withCuda: boolean
  removeWavFileAfterTranscription: false
  whisperOptions: { language?: string; outputInJsonFull: true }
}

type Detailed = typeof import('nodejs-whisper').transcribeDetailed
type Transcriber = (filePath: string, options: Parameters<Detailed>[1]) => Promise<Pick<Awaited<ReturnType<Detailed>>, 'segments'>>

function findWhisperPackageRoot(): string {
  let directory = dirname(require.resolve('nodejs-whisper'))
  while (true) {
    const packageJsonPath = join(directory, 'package.json')
    if (existsSync(packageJsonPath)) {
      const metadata = JSON.parse(readFileSync(packageJsonPath, 'utf8')) as { name?: unknown }
      if (metadata.name === 'nodejs-whisper') return directory
    }
    const parentDirectory = dirname(directory)
    if (parentDirectory === directory) throw new Error('Could not locate nodejs-whisper package root')
    directory = parentDirectory
  }
}

export function detectCudaAddon(exists = existsSync): boolean {
  return exists(join(findWhisperPackageRoot(), 'build', 'cuda', 'Release', 'nodejs_whisper.node'))
}

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
  const whisper = await import('nodejs-whisper')
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
