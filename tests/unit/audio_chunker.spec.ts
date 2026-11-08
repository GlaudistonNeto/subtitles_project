import { test } from '@japa/runner'
import { computeOffsets, splitWav } from '../../app/services/audio_chunker.js'

function fakeDeps(failProbe = false) {
  const calls: Array<{ file: string; args: string[] }> = []
  let directory = ''
  let removed = ''
  const deps: NonNullable<Parameters<typeof splitWav>[2]> = {
    execFile: async (file, args) => {
      calls.push({ file, args })
      if (file === 'ffprobe' && failProbe) throw new Error('ffprobe failed')
      const path = args[args.length - 1] ?? ''
      const duration = path.includes('0000') ? '30.080' : path.includes('0001') ? '29.952' : '30.080'
      return { stdout: file === 'ffprobe' ? duration : '' }
    },
    fs: {
      mkdir: async (path) => { directory = path },
      readdir: async () => ['chunk_0002.wav', 'chunk_0000.wav', 'chunk_0001.wav'],
      rm: async (path) => { removed = path },
    },
  }
  return { deps, calls, get directory() { return directory }, get removed() { return removed } }
}

test('computes offsets from actual durations', ({ assert }) => {
  assert.deepEqual(computeOffsets([30080, 29952, 30080]), [0, 30080, 60032])
})

test('passes segment options to ffmpeg as arguments', async ({ assert }) => {
  const state = fakeDeps()
  await splitWav('input.wav', { chunkSeconds: 30 }, state.deps)
  const args = state.calls[0]!.args
  assert.equal(args[args.indexOf('-f') + 1], 'segment')
  assert.equal(args[args.indexOf('-segment_time') + 1], '30')
  assert.equal(args[args.indexOf('-c') + 1], 'copy')
  assert.isTrue(args[args.length - 1]!.endsWith('chunk_%04d.wav'))
})

test('uses measured durations for chunk start times', async ({ assert }) => {
  const { chunks } = await splitWav('input.wav', { chunkSeconds: 30 }, fakeDeps().deps)
  assert.deepEqual(chunks.map(({ startMs }) => startMs), [0, 30080, 60032])
})

test('removes its directory when ffprobe fails', async ({ assert }) => {
  const state = fakeDeps(true)
  let message = ''
  try { await splitWav('input.wav', { chunkSeconds: 30 }, state.deps) } catch (error) { message = (error as Error).message }
  assert.equal(message, 'ffprobe failed')
  assert.equal(state.removed, state.directory)
})

test('cleanup removes the temporary directory', async ({ assert }) => {
  const state = fakeDeps()
  const result = await splitWav('input.wav', { chunkSeconds: 30 }, state.deps)
  await result.cleanup()
  assert.equal(state.removed, state.directory)
})
