import { existsSync } from 'node:fs'
import { writeFile } from 'node:fs/promises'
import { test } from '@japa/runner'
import SubtitlesController from '../../app/controllers/subtitles_controller.js'
import type { PipelineDeps, PipelineOptions } from '../../app/services/subtitle_pipeline.js'

type Pipeline = (video: string, options: PipelineOptions, deps: PipelineDeps) => Promise<string>
const controller = SubtitlesController.prototype as unknown as { runPipeline: Pipeline }
const originalPipeline = controller.runPipeline
const temporaries: string[] = []
let failPipeline = false
let blocked: { wait: Promise<void>; entered: () => void } | undefined

test.group('POST /subtitles', (group) => {
  group.each.setup(() => {
    failPipeline = false
    blocked = undefined
    temporaries.length = 0
    controller.runPipeline = async (_video, options) => {
      const output = options.outputPath!
      temporaries.push(_video, output)
      if (blocked) { const current = blocked; current.entered(); await current.wait }
      if (failPipeline) throw new Error('internal-secret')
      await writeFile(output, '1\n00:00:00,000 --> 00:00:01,000\nHello\n')
      return output
    }
    return () => { controller.runPipeline = originalPipeline }
  })

  test('returns 400 without a file or for an unsupported extension', async ({ client, assert }) => {
    assert.equal((await client.post('/subtitles').withCsrfToken().send()).status(), 400)
    assert.equal((await client.post('/subtitles').withCsrfToken().file('video', Buffer.from('x'), { filename: 'clip.exe' }).send()).status(), 400)
  })

  test('allows a valid upload after rejecting a forbidden extension', async ({ client, assert }) => {
    const rejected = await client.post('/subtitles').withCsrfToken().file('video', Buffer.from('x'), { filename: 'clip.exe' }).send()
    assert.equal(rejected.status(), 400)
    const accepted = await client.post('/subtitles').withCsrfToken().file('video', Buffer.alloc(5000), { filename: 'clip.mp4', contentType: 'video/mp4' }).send()
    assert.equal(accepted.status(), 200, accepted.text())
  })

  test('downloads the SRT and removes temporary files', async ({ client, assert }) => {
    const response = await client.post('/subtitles').withCsrfToken().file('video', Buffer.alloc(5000), { filename: 'my clip.mp4', contentType: 'video/mp4' }).send()
    assert.equal(response.status(), 200, response.text())
    assert.equal(response.text(), '1\n00:00:00,000 --> 00:00:01,000\nHello\n')
    assert.include(response.header('content-disposition')!, 'attachment; filename="legenda.en.srt"')
    assert.include(response.header('content-disposition')!, "filename*=UTF-8''my%20clip.en.srt")
    assert.isTrue(temporaries.length === 2 && temporaries.every((path) => !existsSync(path)))
  })

  test('supports UTF-8 original filenames in the download header', async ({ client, assert }) => {
    const response = await client.post('/subtitles').withCsrfToken().file('video', Buffer.alloc(5000), { filename: '動画.mp4', contentType: 'video/mp4' }).send()
    assert.equal(response.status(), 200, response.text())
    assert.include(response.header('content-disposition')!, "filename*=UTF-8''")
  })

  test('hides pipeline errors and removes temporary files', async ({ client, assert }) => {
    failPipeline = true
    const response = await client.post('/subtitles').withCsrfToken().file('video', Buffer.alloc(5000), { filename: 'clip.mp4', contentType: 'video/mp4' }).send()
    assert.equal(response.status(), 500)
    assert.notInclude(response.text(), 'internal-secret')
    assert.isTrue(temporaries.every((path) => !existsSync(path)))
  })

  test('rejects a second upload while processing', async ({ client, assert }) => {
    let enter!: () => void
    let release!: () => void
    const entered = new Promise<void>((resolve) => { enter = resolve })
    const wait = new Promise<void>((resolve) => { release = resolve })
    blocked = { wait, entered: enter }
    const first = client.post('/subtitles').withCsrfToken().file('video', Buffer.alloc(5000), { filename: 'first.mp4', contentType: 'video/mp4' }).send()
    await entered
    const second = await client.post('/subtitles').withCsrfToken().file('video', Buffer.alloc(5000), { filename: 'second.mp4', contentType: 'video/mp4' }).send()
    assert.equal(second.status(), 503)
    release()
    assert.equal((await first).status(), 200)
  })
})