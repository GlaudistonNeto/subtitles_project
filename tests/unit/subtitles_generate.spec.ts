import { test } from '@japa/runner'
import ace from '@adonisjs/core/services/ace'
import env from '../../start/env.js'
import SubtitlesGenerateCommand from '../../commands/subtitles_generate.js'
import type { PipelineDeps, PipelineOptions } from '../../app/services/subtitle_pipeline.js'

const calls: Array<{ video: string; options: PipelineOptions; deps: PipelineDeps }> = []
let failPipeline = false

class FakePipelineCommand extends SubtitlesGenerateCommand {
  protected override async runPipeline(video: string, options: PipelineOptions, deps: PipelineDeps) {
    calls.push({ video, options, deps })
    if (failPipeline) throw new Error('test failure')
    return options.outputPath ?? `${video}.en.srt`
  }
}

test.group('subtitles:generate', (group) => {
  group.each.setup(() => {
    calls.length = 0
    failPipeline = false
    ace.ui.switchMode('raw')
    return () => ace.ui.switchMode('normal')
  })

  test('uses env-backed defaults', async ({ assert }) => {
    const command = await ace.create(FakePipelineCommand, ['clip.mp4'])
    await command.exec()
    command.assertSucceeded()
    assert.equal(calls[0].video, 'clip.mp4')
    assert.equal(calls[0].options.language, 'auto')
    assert.equal(calls[0].options.ollamaModel, env.get('OLLAMA_MODEL'))
    assert.equal(calls[0].options.whisperModel, env.get('WHISPER_MODEL'))
    assert.equal(calls[0].deps.deeplApiKey, env.get('DEEPL_API_KEY'))
  })

  test('lets flags override defaults', async ({ assert }) => {
    const command = await ace.create(FakePipelineCommand, ['clip.mp4', '-l', 'ja', '--skip-refine', '-o', 'custom.srt', '--ollama-model', 'ollama-test', '--whisper-model', 'whisper-test', '--keep-audio'])
    await command.exec()
    command.assertSucceeded()
    assert.deepEqual(calls[0].options, {
      language: 'ja', skipRefine: true, outputPath: 'custom.srt', ollamaModel: 'ollama-test',
      whisperModel: 'whisper-test', keepAudio: true,
    })
  })

  test('rejects unsupported language values', async ({ assert }) => {
    const command = await ace.create(FakePipelineCommand, ['clip.mp4', '--language', 'fr'])
    await command.exec()
    assert.equal(command.exitCode, 1)
    assert.isEmpty(calls)
    command.assertLog('[ red(error) ] Invalid --language value. Use ja, zh, or auto.')
  })

  test('exits unsuccessfully when the pipeline fails', async ({ assert }) => {
    failPipeline = true
    const command = await ace.create(FakePipelineCommand, ['clip.mp4'])
    await command.exec()
    assert.equal(command.exitCode, 1)
    command.assertLog('[ red(error) ] Subtitle generation failed. No SRT was produced.')
  })
})