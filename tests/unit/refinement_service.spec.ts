import { test } from '@japa/runner'
import { refineSegments } from '../../app/services/refinement_service.js'
import type { Segment } from '../../app/types/subtitles.js'

const segments = (count: number): Segment[] => Array.from({ length: count }, (_, index) => ({
  index: index + 1, startMs: index * 1000, endMs: index * 1000 + 500, text: `line ${index}`,
}))
const response = (content: string) => new Response(JSON.stringify({ message: { content } }))
const call = (fetcher: typeof fetch, input = segments(1)) =>
  refineSegments(input, 'ja', 'test-model', 'http://ollama.test/', fetcher)
const readInput = (init?: RequestInit): string[] => {
  const body = JSON.parse(String(init?.body))
  const prompt = body.messages[0].content as string
  return JSON.parse(prompt.slice(prompt.lastIndexOf('INPUT_JSON:') + 11))
}

test('batches 20 segments, supplies previous context, and preserves timestamps', async ({ assert }) => {
  const requests: string[][] = []
  const prompts: string[] = []
  const input = segments(20)
  const result = await refineSegments(input, 'ja', 'model', 'http://ollama.test', async (_url, init) => {
    const body = JSON.parse(String(init?.body))
    prompts.push(body.messages[0].content)
    const texts = readInput(init)
    requests.push(texts)
    return response(JSON.stringify({ lines: texts.map((text) => `fixed ${text}`) }))
  })
  assert.deepEqual(requests.map(({ length }) => length), [8, 8, 4])
  assert.include(prompts[1], `CONTEXT_JSON:${JSON.stringify(requests[0].map((text) => `fixed ${text}`))}`)
  assert.lengthOf(result, input.length)
  assert.include(result[8].text, 'fixed line 8')
  assert.deepEqual(result.map(({ startMs, endMs }) => [startMs, endMs]), input.map(({ startMs, endMs }) => [startMs, endMs]))
})

test('keeps original text after a network failure', async ({ assert }) => {
  const input = segments(2)
  const result = await call(async () => { throw new Error('offline') }, input)
  assert.deepEqual(result.map(({ text }) => text), input.map(({ text }) => text))
})

test('sends a structured schema for lines', async ({ assert }) => {
  let format: unknown
  await call(async (_url, init) => {
    format = JSON.parse(String(init?.body)).format
    return response(JSON.stringify({ lines: ['fixed line 0'] }))
  })
  assert.deepEqual(format, {
    type: 'object', properties: { lines: { type: 'array', items: { type: 'string' } } }, required: ['lines'],
  })
})

test('keeps original text when the response is an array at the root', async ({ assert }) => {
  const input = segments(1)
  const result = await call(async () => response(' ["fixed line 0"] '), input)
  assert.deepEqual(result.map(({ text }) => text), input.map(({ text }) => text))
})

test('sets keep_alive to zero only on the last request', async ({ assert }) => {
  const keepAliveValues: unknown[] = []
  await call(async (_url, init) => {
    const body = JSON.parse(String(init?.body))
    keepAliveValues.push(body.keep_alive)
    return response(JSON.stringify({ lines: readInput(init) }))
  }, segments(20))
  assert.deepEqual(keepAliveValues, [undefined, undefined, 0])
})

test('keeps original text when the response has the wrong number of lines', async ({ assert }) => {
  const input = segments(2)
  const result = await call(async () => response('{"lines":["only one"]}'), input)
  assert.deepEqual(result.map(({ text }) => text), input.map(({ text }) => text))
})

test('keeps original text when the response is invalid JSON', async ({ assert }) => {
  const input = segments(1)
  const result = await call(async () => response('not json'), input)
  assert.deepEqual(result.map(({ text }) => text), input.map(({ text }) => text))
})
