import { randomUUID } from 'node:crypto'
import { mkdir, readFile, unlink } from 'node:fs/promises'
import { basename, join, parse } from 'node:path'
import type { HttpContext } from '@adonisjs/core/http'
import env from '#start/env'
import { buildPipelineDeps } from '#services/pipeline_deps'
import { runPipeline, type PipelineDeps, type PipelineOptions } from '#services/subtitle_pipeline'

let processing = false
const extensions = ['mp4', 'mkv', 'mov', 'webm', 'avi', 'mp3', 'wav', 'm4a', 'flac', 'ogg']

export default class SubtitlesController {
  protected runPipeline(videoPath: string, options: PipelineOptions, deps: PipelineDeps) {
    return runPipeline(videoPath, options, deps)
  }

  async store({ request, response }: HttpContext) {
    if (processing) return response.status(503).send('A subtitle is already being processed. Please try again shortly.')
    let uploadPath: string | undefined
    let outputPath: string | undefined
    try {
      const upload = request.file('video', { size: `${env.get('MAX_UPLOAD_MB')}mb`, extnames: extensions })
      if (!upload) return response.badRequest('The video field is required.')
      uploadPath = upload.tmpPath ?? undefined
      if (!upload.isValid) return response.badRequest({ errors: upload.errors })
      const language = request.input('language', 'auto')
      if (!['ja', 'zh', 'auto'].includes(language)) return response.badRequest('Invalid language.')
      const skipValue = request.input('skipRefine')
      if (skipValue != null && skipValue !== '' && ![true, false, 'true', 'false', '1', '0', 'on', 'off'].includes(skipValue)) {
        return response.badRequest('Invalid skipRefine value.')
      }
      const skipRefine = skipValue === true || ['true', '1', 'on'].includes(skipValue)
      processing = true
      const uploadDir = join(process.cwd(), 'tmp', 'uploads')
      await mkdir(uploadDir, { recursive: true })
      const storedName = `${randomUUID()}.${upload.extname}`
      await upload.move(uploadDir, { name: storedName })
      uploadPath = join(uploadDir, storedName)
      outputPath = join(uploadDir, `${randomUUID()}.srt`)
      await this.runPipeline(uploadPath, {
        language, skipRefine, outputPath,
        ollamaModel: env.get('OLLAMA_MODEL'), whisperModel: env.get('WHISPER_MODEL'),
      }, buildPipelineDeps(() => {}))
      const originalBase = parse(basename(upload.clientName.replace(/[\r\n"\\/]/g, ''))).name
      const subtitle = await readFile(outputPath)
      response.header('Content-Disposition', `attachment; filename="legenda.en.srt"; filename*=UTF-8''${encodeURIComponent(originalBase)}.en.srt`)
      response.header('Content-Type', 'application/x-subrip; charset=utf-8').send(subtitle)
    } catch {
      return response.status(500).send('Subtitle processing failed. Please try again.')
    } finally {
      processing = false
      await Promise.all([uploadPath, outputPath].filter((path): path is string => Boolean(path)).map((path) => unlink(path).catch(() => {})))
    }
  }
}