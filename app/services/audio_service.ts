import { execFile } from 'node:child_process'
import { mkdir, rm } from 'node:fs/promises'
import { randomUUID } from 'node:crypto'
import { promisify } from 'node:util'
import { join } from 'node:path'

const execFileAsync = promisify(execFile)
type ProcessExecutor = (file: string, args: string[]) => Promise<void>
type AudioConsumer<T> = (wavPath: string) => Promise<T>

const runExecFile: ProcessExecutor = async (file, args) => {
  await execFileAsync(file, args, { shell: false, maxBuffer: 10 * 1024 * 1024 })
}

export class AudioService {
  constructor(private readonly execute: ProcessExecutor = runExecFile) {}

  async withAudio<T>(videoPath: string, consumer: AudioConsumer<T>, keepAudio = false): Promise<T> {
    const temporaryDirectory = join(process.cwd(), 'tmp')
    await mkdir(temporaryDirectory, { recursive: true })
    const wavPath = join(temporaryDirectory, `${randomUUID()}.wav`)

    try {
      await this.execute('ffmpeg', [
        '-nostdin',
        '-loglevel', 'error',
        '-i', videoPath,
        '-vn',
        '-af', 'highpass=f=80,loudnorm',
        '-ar', '16000',
        '-ac', '1',
        '-c:a', 'pcm_s16le',
        wavPath,
      ])
      return await consumer(wavPath)
    } finally {
      if (!keepAudio) await rm(wavPath, { force: true })
    }
  }
}