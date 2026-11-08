import { test } from '@japa/runner'
import { collapseRepeats } from '../../app/services/segment_filters.js'
import { resolveCuda } from '../../app/services/transcription_service.js'
import { runPipeline, type PipelineDeps } from '../../app/services/subtitle_pipeline.js'
import type { Segment } from '../../app/types/subtitles.js'

function makeSegments(texts: string[]): Segment[] {
  return texts.map((text, index) => ({ index: index + 1, startMs: index * 100, endMs: index * 100 + 50, text }))
}

test('collapses ten identical segments to one', ({ assert }) => {
  assert.deepEqual(collapseRepeats(makeSegments(Array(10).fill('A'))).map(({ text }) => text), ['A'])
})

test('collapses a repeated two-segment period', ({ assert }) => {
  const texts = Array.from({ length: 10 }, () => ['A。', 'B、']).flat()
  assert.deepEqual(collapseRepeats(makeSegments(texts)).map(({ text }) => text), ['A。', 'B、'])
})

test('collapses a repeated three-segment period', ({ assert }) => {
  const texts = Array.from({ length: 5 }, () => ['A', 'B', 'C']).flat()
  assert.deepEqual(collapseRepeats(makeSegments(texts)).map(({ text }) => text), ['A', 'B', 'C'])
})

test('keeps only three occurrences uncollapsed', ({ assert }) => {
  const segments = makeSegments(['A', 'A', 'A'])
  assert.deepEqual(collapseRepeats(segments), segments)
})

test('leaves normal text intact', ({ assert }) => {
  const segments = makeSegments(['First', 'Second', 'Third', 'Fourth'])
  assert.deepEqual(collapseRepeats(segments), segments)
})

test('preserves retained times and renumbers indexes', ({ assert }) => {
  const segments = makeSegments(['Intro', ...Array(10).fill('A')])
  const result = collapseRepeats(segments)
  assert.deepEqual(result.map(({ index, startMs, endMs }) => [index, startMs, endMs]), [[1, 0, 50], [2, 100, 150]])
})

test('filters repeated segments before refinement and translation', async ({ assert }) => {
  const repeated = makeSegments(Array(10).fill('A'))
  let refined: Segment[] = []
  let translated: Segment[] = []
  const deps: PipelineDeps = {
    audio: { withAudio: async (_video, consume) => consume('audio.wav') },
    transcribe: async () => repeated,
    refine: async (segments) => { refined = segments; return segments },
    translate: async (segments) => { translated = segments; return segments },
    generateSrt: () => '', writeFile: async () => {}, detectCudaAddon: () => false, resolveCuda,
    deeplApiKey: '', modelRootPath: '/models', logger: { warn: () => {} },
  }
  await runPipeline('clip.mp4', { language: 'ja', skipRefine: false, ollamaModel: 'test', whisperModel: 'test' }, deps)
  assert.deepEqual(refined.map(({ text }) => text), ['A'])
  assert.deepEqual(translated.map(({ text }) => text), ['A'])
})