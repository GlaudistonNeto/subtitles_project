import { randomUUID } from 'node:crypto'
import { execFile } from 'node:child_process'
import { mkdir, readdir, rm } from 'node:fs/promises'
import { join } from 'node:path'
import { promisify } from 'node:util'

const execFileAsync = promisify(execFile)
type Deps = {
  execFile: (file: string, args: string[]) => Promise<{ stdout: string | Buffer }>
  fs: {
    mkdir: (path: string) => Promise<unknown>
    readdir: (path: string) => Promise<string[]>
    rm: (path: string) => Promise<unknown>
  }
}
const defaultDeps: Deps = {
  execFile: async (file, args) => execFileAsync(file, args, { shell: false, encoding: 'utf8' }),
  fs: {
    mkdir: async (path) => mkdir(path, { recursive: true }),
    readdir: async (path) => readdir(path),
    rm: async (path) => rm(path, { recursive: true, force: true }),
  },
}

export function computeOffsets(durationsMs: number[]): number[] {
  let offset = 0
  return durationsMs.map((duration) => {
    const start = offset
    offset += duration
    return start
  })
}

export async function splitWav(
  wavPath: string, options: { chunkSeconds: number }, deps: Deps = defaultDeps,
): Promise<{ chunks: { path: string; startMs: number; durationMs: number }[]; cleanup(): Promise<void> }> {
  const directory = join(process.cwd(), 'tmp', randomUUID())
  await deps.fs.mkdir(directory)
  try {
    await deps.execFile('ffmpeg', ['-nostdin', '-loglevel', 'error', '-i', wavPath, '-f', 'segment',
      '-segment_time', String(options.chunkSeconds), '-c', 'copy', '-reset_timestamps', '1',
      join(directory, 'chunk_%04d.wav')])
    const files = (await deps.fs.readdir(directory)).filter((file) => /^chunk_\d+\.wav$/.test(file)).sort()
    const measured = []
    for (const file of files) {
      const path = join(directory, file)
      const result = await deps.execFile('ffprobe', ['-v', 'error', '-show_entries', 'format=duration', '-of', 'csv=p=0', path])
      measured.push({ path, durationMs: Math.round(Number.parseFloat(result.stdout.toString().trim()) * 1000) })
    }
    const offsets = computeOffsets(measured.map(({ durationMs }) => durationMs))
    return { chunks: measured.map((chunk, index) => ({ ...chunk, startMs: offsets[index]! })), cleanup: () => deps.fs.rm(directory).then(() => undefined) }
  } catch (error) {
    try { await deps.fs.rm(directory) } catch {}
    throw error
  }
}
