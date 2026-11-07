import { test } from '@japa/runner'
import { translateSegments } from '../../app/services/translation_service.js'
import type { Segment } from '../../app/types/subtitles.js'

const segments = (count: number): Segment[] => Array.from({ length: count }, (_, index) => ({
  index: index + 1, startMs: index * 1000, endMs: index * 1000 + 500, text: `line ${index}`,
}))
const response = (texts: string[]) => new Response(JSON.stringify({ translations: texts.map((text) => ({ text })) }))
const body = (init?: RequestInit) => JSON.parse(String(init?.body))

test('batches 65 texts, uses neighboring context only, and preserves timestamps', async ({ assert }) => {
  const input = segments(65)
  const calls: ReturnType<typeof body>[] = []
  const result = await translateSegments(input, 'ja', 'key:fx', async (_url, init) => {
    const data = body(init); calls.push(data); return response(data.text.map((text: string) => `EN ${text}`))
  })
  assert.deepEqual(calls.map(({ text }) => text.length), [30, 30, 5])
  assert.include(calls[0].context, 'line 30')
  assert.notInclude(calls[0].context, 'line 0\n')
  assert.include(calls[1].context, 'line 0')
  assert.include(calls[1].context, 'line 60')
  assert.notInclude(calls[1].context, 'line 30\n')
  assert.deepEqual(result.map(({ startMs, endMs }) => [startMs, endMs]), input.map(({ startMs, endMs }) => [startMs, endMs]))
})

test('retries 429 and succeeds, without real waiting', async ({ assert }) => {
  let attempts = 0
  const delays: number[] = []
  const result = await translateSegments(segments(1), 'zh', 'key', async () => {
    attempts++
    return attempts === 1 ? new Response('', { status: 429 }) : response(['translated'])
  }, async (ms) => { delays.push(ms) })
  assert.equal(result[0].text, 'translated')
  assert.equal(attempts, 2)
  assert.deepEqual(delays, [250])
})

test('throws clear quota and result-count errors without exposing the key', async ({ assert }) => {
  const key = 'secret:fx'
  let message = ''
  try { await translateSegments(segments(1), 'ja', key, async () => new Response('', { status: 456 })) }
  catch (error) { message = String(error) }
  assert.include(message, 'quota')
  assert.notInclude(message, key)
  await assert.rejects(() => translateSegments(segments(2), 'ja', key,
    async () => response(['one'])), /unexpected number/)
})

test('selects host and omits source language for auto', async ({ assert }) => {
  const calls: Array<{ url: string; data: ReturnType<typeof body> }> = []
  for (const [key, host] of [['free:fx', 'api-free'], ['paid', 'api']] as const) {
    await translateSegments(segments(1), 'auto', key, async (url, init) => {
      calls.push({ url: String(url), data: body(init) }); return response(['ok'])
    })
    assert.include(calls.at(-1)!.url, `https://${host}.deepl.com/v2/translate`)
    assert.isUndefined(calls.at(-1)!.data.source_lang)
  }
})
