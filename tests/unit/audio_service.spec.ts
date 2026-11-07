import { access, unlink, writeFile } from 'node:fs/promises'
import { test } from '@japa/runner'
import { AudioService } from '../../app/services/audio_service.js'

async function exists(path: string): Promise<boolean> {
  try {
    await access(path)
    return true
  } catch {
    return false
  }
}

test('passes the required FFmpeg arguments as separate values', async ({ assert }) => {
  let command = ''
  let args: string[] = []
  const service = new AudioService(async (file, values) => {
    command = file
    args = values
  })
  await service.withAudio('video "with spaces".mp4', async () => undefined)
  assert.equal(command, 'ffmpeg')
  assert.equal(args[0], '-nostdin')
  assert.equal(args[1], '-loglevel')
  assert.equal(args[2], 'error')
  assert.equal(args[3], '-i')
  assert.equal(args[4], 'video "with spaces".mp4')
  assert.equal(args[args.indexOf('-ar') + 1], '16000')
  assert.equal(args[args.indexOf('-ac') + 1], '1')
  assert.include(args, 'highpass=f=80,loudnorm')
})

test('does not pass filters or options that shorten silence', async ({ assert }) => {
  let args: string[] = []
  const service = new AudioService(async (_file, values) => { args = values })
  await service.withAudio('video.mp4', async () => undefined)
  assert.isFalse(args.some((value) => value.includes('silenceremove')))
  assert.isFalse(args.some((value) => value.includes('atrim')))
  assert.equal(args.indexOf('-ss'), -1)
})

test('removes the temporary WAV when FFmpeg fails', async ({ assert }) => {
  let wavPath = ''
  const service = new AudioService(async (_file, args) => {
    wavPath = args[args.length - 1]!
    await writeFile(wavPath, 'wav')
    throw new Error('ffmpeg failed')
  })
  let rejected = false
  try {
    await service.withAudio('video.mp4', async () => undefined)
  } catch {
    rejected = true
  }
  assert.isTrue(rejected)
  assert.isFalse(await exists(wavPath))
})

test('keeps the temporary WAV when keepAudio is true', async ({ assert }) => {
  let wavPath = ''
  const service = new AudioService(async (_file, args) => {
    wavPath = args[args.length - 1]!
    await writeFile(wavPath, 'wav')
  })
  await service.withAudio('video.mp4', async () => undefined, true)
  assert.isTrue(await exists(wavPath))
  await unlink(wavPath)
})