import { test } from '@japa/runner'
import { resolveCuda } from '../../app/services/transcription_service.js'
import { runPipeline, type PipelineDeps, type PipelineOptions } from '../../app/services/subtitle_pipeline.js'
import type { Segment } from '../../app/types/subtitles.js'

const options: PipelineOptions = { language: 'ja', skipRefine: true, ollamaModel: 'test', whisperModel: 'test' }
function harness(overrides: Partial<PipelineDeps> = {}) {
  const logs: string[] = []
  const deps: PipelineDeps = {
    audio: { withAudio: async (_video, consume) => consume('audio.wav') },
    splitWav: async (path) => ({ chunks: [{ path, startMs: 0, durationMs: 40000 }], cleanup: async () => {} }),
    transcribe: async () => [], refine: async (segments) => segments,
    translate: async (segments) => segments, generateSrt: () => '', writeFile: async () => {},
    detectCudaAddon: () => false, resolveCuda, deeplApiKey: '', modelRootPath: '/models',
    logger: { warn: () => {}, info: (message) => logs.push(message) }, ...overrides,
  }
  return { deps, logs }
}

test('offsets and renumbers ordered chunks and logs progress', async ({ assert }) => {
  const offsets = [0, 600064, 1199936]
  const generated: Segment[] = []
  const state = harness({
    splitWav: async () => ({ chunks: offsets.map((startMs, i) => ({ path: `chunk-${i}`, startMs, durationMs: 1 })), cleanup: async () => {} }),
    transcribe: async (path) => [{ index: 90, startMs: 1000, endMs: 1100, text: path }],
    generateSrt: (segments) => { generated.push(...segments); return '' },
  })
  await runPipeline('clip.mp4', options, state.deps)
  assert.deepEqual(generated.map(({ startMs }) => startMs), [1000, 601064, 1200936])
  assert.deepEqual(generated.map(({ index, text }) => [index, text]), [[1, 'chunk-0'], [2, 'chunk-1'], [3, 'chunk-2']])
  assert.deepEqual(state.logs, ['Pedaço 1/3', 'Pedaço 2/3', 'Pedaço 3/3'])
})

test('cleans chunks and propagates a transcription error', async ({ assert }) => {
  let cleaned = false
  const state = harness({
    splitWav: async () => ({ chunks: ['one', 'two'].map((path) => ({ path, startMs: 0, durationMs: 1 })), cleanup: async () => { cleaned = true } }),
    transcribe: async (path) => { if (path === 'two') throw new Error('chunk failed'); return [] },
  })
  let message = ''
  try { await runPipeline('clip.mp4', options, state.deps) } catch (error) { message = (error as Error).message }
  assert.equal(message, 'chunk failed')
  assert.isTrue(cleaned)
})

test('collapses repeats within a chunk before translation', async ({ assert }) => {
  let translated: Segment[] = []
  const state = harness({
    transcribe: async () => Array.from({ length: 10 }, (_, index) => ({ index, startMs: index, endMs: index + 1, text: 'Loop' })),
    translate: async (segments) => { translated = segments; return segments },
  })
  await runPipeline('clip.mp4', options, state.deps)
  assert.deepEqual(translated.map(({ text }) => text), ['Loop'])
})
