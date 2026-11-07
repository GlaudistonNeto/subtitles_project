import { test } from '@japa/runner'
import { buildWhisperOptions, resolveCuda, transcribe } from '../../app/services/transcription_service.js'

const options = buildWhisperOptions({ language: 'auto', modelName: 'base', modelRootPath: '/models', withCuda: false })

test('preserves every segment timestamp without offsets', async ({ assert }) => {
  const result = await transcribe('audio.wav', options, async () => ({ segments: [
    { start: 5_000, end: 5_480, text: 'First' },
    { start: 6_125, end: 7_000, text: 'Second' },
  ] }))
  assert.deepEqual(result.map(({ startMs, endMs }) => [startMs, endMs]), [[5_000, 5_480], [6_125, 7_000]])
  assert.equal(result[0].startMs, 5_000)
})

test('discards empty text and assigns sequential indices', async ({ assert }) => {
  const result = await transcribe('audio.wav', options, async () => ({ segments: [
    { start: 0, end: 1, text: '  ' },
    { start: 2, end: 3, text: 'Kept' },
  ] }))
  assert.deepEqual(result, [{ index: 1, startMs: 2, endMs: 3, text: 'Kept' }])
})

test('falls back to CPU only when CUDA is requested but unavailable', ({ assert }) => {
  const unavailable = resolveCuda({ requested: true, cudaAddonAvailable: false })
  assert.isFalse(unavailable.withCuda)
  assert.isString(unavailable.warning)
  assert.deepEqual(resolveCuda({ requested: true, cudaAddonAvailable: true }), { withCuda: true })
})

test('forces Japanese while leaving auto detection unset', ({ assert }) => {
  const japanese = buildWhisperOptions({ language: 'ja', modelName: 'base', modelRootPath: '/models', withCuda: false })
  const automatic = buildWhisperOptions({ language: 'auto', modelName: 'base', modelRootPath: '/models', withCuda: false })
  assert.equal(japanese.whisperOptions.language, 'ja')
  assert.isUndefined(automatic.whisperOptions.language)
  assert.isFalse(automatic.removeWavFileAfterTranscription)
})
