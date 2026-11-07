import { test } from '@japa/runner'
import { generateSrt } from '../../app/services/srt_service.js'
import type { Segment } from '../../app/types/subtitles.js'

function parseCues(srt: string) {
  return srt.split('\n\n').map((cue) => {
    const [index, timing, ...lines] = cue.split('\n')
    const [start, end] = timing.split(' --> ')
    return { index: Number(index), start, end, lines }
  })
}

function milliseconds(value: string) {
  const [clock, fraction] = value.split(',')
  return clock.split(':').reduce((total, part) => total * 60 + Number(part), 0) * 1_000 + Number(fraction)
}

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

test('splits long English segments proportionally into sequential cues', ({ assert }) => {
  const cues = parseCues(generateSrt([{ index: 1, startMs: 5_000, endMs: 15_000, text: 'word '.repeat(20) }]))
  assert.equal(cues.length, 2)
  assert.deepEqual(cues.map(({ index }) => index), [1, 2])
  assert.equal(cues[0].start, '00:00:05,000')
  assert.equal(cues[1].end, '00:00:15,000')
  assert.equal(cues[0].end, cues[1].start)
  assert.equal(milliseconds(cues[0].end), 5_000 + Math.round(10_000 * 78 / 97))
  for (const cue of cues) assert.isTrue(cue.lines.length <= 2 && cue.lines.every((line) => Array.from(line).length <= 42))
})

test('splits long Japanese segments into cues within the line limit', ({ assert }) => {
  const cues = parseCues(generateSrt([{ index: 1, startMs: 0, endMs: 10_000, text: 'あ'.repeat(100) }]))
  assert.equal(cues.length, 2)
  assert.equal(cues[0].end, '00:00:08,400')
  assert.equal(cues[1].start, cues[0].end)
  assert.equal(cues[1].end, '00:00:10,000')
  for (const cue of cues) assert.isTrue(cue.lines.length <= 2 && cue.lines.every((line) => Array.from(line).length <= 42))
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
