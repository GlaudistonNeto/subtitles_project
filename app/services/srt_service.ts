import type { Segment } from '../types/subtitles.js'

const CJK = /[\u3040-\u30ff\u3400-\u4dbf\u4e00-\u9fff]/u
const CJK_PUNCTUATION = /[、。，！？]/u

function wrapText(text: string): string[] {
  if (CJK.test(text)) {
    const characters = Array.from(text.replace(/\s+/gu, ''))
    const lines: string[] = []
    while (characters.length) {
      let length = Math.min(42, characters.length)
      if (characters.length > 42) {
        for (let i = 0; i < 42; i++) if (CJK_PUNCTUATION.test(characters[i])) length = i + 1
      }
      lines.push(characters.splice(0, length).join(''))
    }
    return lines
  }

  const words = text.trim().split(/\s+/u).flatMap((word) => {
    const characters = Array.from(word)
    const pieces: string[] = []
    while (characters.length > 42) pieces.push(characters.splice(0, 42).join(''))
    if (characters.length) pieces.push(characters.join(''))
    return pieces
  })
  const lines: string[] = []
  for (const word of words) {
    const last = lines.length - 1
    if (last >= 0 && Array.from(lines[last]).length + 1 + Array.from(word).length <= 42) {
      lines[last] += ` ${word}`
    } else {
      lines.push(word)
    }
  }
  return lines
}

function timestamp(ms: number): string {
  const value = Math.max(0, Math.floor(ms))
  const hours = Math.floor(value / 3_600_000)
  const minutes = Math.floor((value % 3_600_000) / 60_000)
  const seconds = Math.floor((value % 60_000) / 1_000)
  const milliseconds = value % 1_000
  return `${String(hours).padStart(2, '0')}:${String(minutes).padStart(2, '0')}:${String(seconds).padStart(2, '0')},${String(milliseconds).padStart(3, '0')}`
}

export function generateSrt(segments: Segment[]): string {
  const cues = segments.filter((segment) => segment.text.trim().length > 0).flatMap((segment) => {
    const lines = wrapText(segment.text)
    const parts: string[] = []
    for (let index = 0; index < lines.length; index += 2) parts.push(lines.slice(index, index + 2).join('\n'))
    if (!parts.length) parts.push('')
    const weights = parts.map((part) => Array.from(part.replace(/\n/gu, '')).length || 1)
    const total = weights.reduce((sum, weight) => sum + weight, 0)
    const duration = segment.endMs - segment.startMs
    let allocated = 0
    return parts.map((text, index) => {
      const startMs = segment.startMs + Math.round(duration * allocated / total)
      allocated += weights[index]
      const endMs = index === parts.length - 1
        ? segment.endMs
        : segment.startMs + Math.round(duration * allocated / total)
      return { startMs, endMs, text, split: parts.length > 1 }
    })
  })
  return cues.map((cue, index) => {
    let endMs = cue.endMs
    if (endMs - cue.startMs < 1_000 && !cue.split) {
      endMs = cue.startMs + 1_000
      if (cues[index + 1]) endMs = Math.max(cue.startMs, Math.min(endMs, cues[index + 1].startMs))
    }
    return `${index + 1}\n${timestamp(cue.startMs)} --> ${timestamp(endMs)}\n${cue.text}`
  }).join('\n\n')
}
