import { test } from '@japa/runner'
import { resolveCuda } from '../../app/services/transcription_service.js'
import { generateSrt } from '../../app/services/srt_service.js'
import { runPipeline, type PipelineDeps, type PipelineOptions } from '../../app/services/subtitle_pipeline.js'
import type { Segment } from '../../app/types/subtitles.js'

const options: PipelineOptions = {
  language: 'ja', skipRefine: false, ollamaModel: 'test-ollama', whisperModel: 'test-whisper',
}
const original: Segment[] = [{ index: 1, startMs: 5000, endMs: 5600, text: '日本語' }]

function harness(overrides: Partial<PipelineDeps> = {}) {
  const events: string[] = []
  const writes: Array<{ path: string; content: string }> = []
  let cleaned = false
  const deps: PipelineDeps = {
    audio: { withAudio: async <T>(_video: string, consume: (path: string) => Promise<T>, keepAudio = false) => {
      events.push('audio')
      try { return await consume('temporary.wav') } finally { if (!keepAudio) cleaned = true }
    } },
    detectCudaAddon: () => { events.push('detectCudaAddon'); return false },
    resolveCuda: (input) => { events.push('resolveCuda'); return resolveCuda(input) },
    transcribe: async () => { events.push('transcribe'); return original },
    refine: async (segments) => { events.push('refine'); return segments },
    translate: async (segments) => { events.push('translate'); return segments.map((segment) => ({ ...segment, text: 'English' })) },
    generateSrt: (segments) => { events.push('generateSrt'); return generateSrt(segments) },
    writeFile: async (path, content) => { events.push('writeFile'); writes.push({ path, content }) },
    deeplApiKey: 'test-key', modelRootPath: '/models', logger: { warn: () => events.push('warn') },
    ...overrides,
  }
  return { deps, events, writes, isCleaned: () => cleaned }
}

test('runs each pipeline stage in order and writes the generated SRT', async ({ assert }) => {
  const state = harness()
  await runPipeline('clip.mp4', options, state.deps)
  assert.deepEqual(state.events, ['detectCudaAddon', 'resolveCuda', 'warn', 'audio', 'transcribe', 'refine', 'translate', 'generateSrt', 'writeFile'])
  assert.include(state.writes[0].content, '00:00:05,000')
})

test('skips refinement and derives the default output path', async ({ assert }) => {
  const state = harness()
  const path = await runPipeline('/videos/clip.mp4', { ...options, skipRefine: true }, state.deps)
  assert.equal(path, '/videos/clip.en.srt')
  assert.isFalse(state.events.includes('refine'))
  assert.equal(state.writes[0].path, path)
})

test('discards empty translations, renumbers segments, and preserves their times', async ({ assert }) => {
  const translatedSegments: Segment[] = [
    { index: 10, startMs: 1_000, endMs: 2_500, text: '' },
    { index: 20, startMs: 3_000, endMs: 4_500, text: '   ' },
    { index: 30, startMs: 5_000, endMs: 7_000, text: 'First' },
    { index: 40, startMs: 8_000, endMs: 10_000, text: 'Second' },
  ]
  let generatedSegments: Segment[] = []
  const state = harness({
    transcribe: async () => translatedSegments,
    translate: async (segments) => segments,
    generateSrt: (segments) => {
      generatedSegments = segments
      return generateSrt(segments)
    },
  })

  await runPipeline('clip.mp4', options, state.deps)

  assert.deepEqual(generatedSegments.map(({ index }) => index), [1, 2])
  assert.deepEqual(generatedSegments.map(({ startMs, endMs }) => [startMs, endMs]), [[5_000, 7_000], [8_000, 10_000]])
  assert.equal(state.writes[0].content, '1\n00:00:05,000 --> 00:00:07,000\nFirst\n\n2\n00:00:08,000 --> 00:00:10,000\nSecond')
})

test('cleans the temporary WAV when translation fails', async ({ assert }) => {
  const state = harness({ translate: async () => { throw new Error('translation failed') } })
  let failed = false
  try { await runPipeline('clip.mp4', options, state.deps) } catch { failed = true }
  assert.isTrue(failed)
  assert.isTrue(state.isCleaned())
})
