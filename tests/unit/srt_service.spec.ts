import { test } from '@japa/runner'
import { generateSrt } from '../../app/services/srt_service.js'
import type { Segment } from '../../app/types/subtitles.js'

test('wraps English subtitles at 42 characters', ({ assert }) => {
  const text = generateSrt([{ index: 4, startMs: 0, endMs: 2_000, text: 'one two three four five six seven eight nine ten eleven twelve' }])
  const lines = text.split('\n').slice(2)
  assert.isTrue(lines.length <= 2)
  assert.isTrue(lines.every((line) => Array.from(line).length <= 42))
})

test('wraps CJK by character count and punctuation, not spaces', ({ assert }) => {
  const text = generateSrt([{ index: 1, startMs: 0, endMs: 2_000, text: `${'あ'.repeat(41)}、${'い'.repeat(4)}` }])
  const lines = text.split('\n').slice(2)
  assert.equal(lines[0], `${'あ'.repeat(41)}、`)
  assert.equal(lines[1], 'いいいい')
})

test('extends only the end for minimum duration without passing the next start', ({ assert }) => {
  const segments: Segment[] = [
    { index: 1, startMs: 2_000, endMs: 2_200, text: 'First' },
    { index: 2, startMs: 2_700, endMs: 4_000, text: 'Next' },
  ]
  const first = generateSrt(segments).split('\n\n')[0]
  assert.include(first, '00:00:02,000 --> 00:00:02,700')
})

test('preserves initial silence and numbers cues sequentially', ({ assert }) => {
  const srt = generateSrt([
    { index: 9, startMs: 5_000, endMs: 5_600, text: 'Speech begins' },
    { index: 20, startMs: 6_500, endMs: 7_500, text: 'Next speech' },
  ])
  assert.include(srt, '1\n00:00:05,000 --> 00:00:06,000\nSpeech begins')
  assert.include(srt, '2\n00:00:06,500 --> 00:00:07,500\nNext speech')
})
