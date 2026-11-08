import type { Segment } from '../types/subtitles.js'

function normalizeText(text: string): string {
  return text.replace(/[\s\p{P}]/gu, '')
}

export function collapseRepeats(segments: Segment[]): Segment[] {
  const normalized = segments.map(({ text }) => normalizeText(text))
  const kept: Segment[] = []
  let position = 0

  while (position < segments.length) {
    let repeatPeriod = 0
    let repeatComparisons = 0
    for (let period = 1; period <= 3 && !repeatPeriod; period++) {
      let comparisons = 0
      while (position + comparisons + period < segments.length &&
        normalized[position + comparisons] === normalized[position + comparisons + period]) comparisons++
      if (comparisons >= 4 * period) {
        repeatPeriod = period
        repeatComparisons = comparisons
      }
    }

    if (repeatPeriod) {
      kept.push(...segments.slice(position, position + repeatPeriod))
      position += repeatPeriod + repeatComparisons
    } else {
      kept.push(segments[position])
      position++
    }
  }

  return kept.map((segment, index) => ({ ...segment, index: index + 1 }))
}
