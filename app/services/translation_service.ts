import type { Segment } from '../types/subtitles.js'

type Wait = (milliseconds: number) => Promise<void>
const defaultWait: Wait = (milliseconds) => new Promise((resolve) => setTimeout(resolve, milliseconds))

export async function translateSegments(
  segments: Segment[], sourceLanguage: string, apiKey: string,
  request: typeof fetch = fetch, wait: Wait = defaultWait,
): Promise<Segment[]> {
  const result = segments.map((segment) => ({ ...segment }))
  const host = apiKey.endsWith(':fx') ? 'https://api-free.deepl.com' : 'https://api.deepl.com'
  const source = sourceLanguage === 'auto' ? undefined : sourceLanguage.toUpperCase()
  for (let start = 0; start < segments.length; start += 30) {
    const end = Math.min(start + 30, segments.length)
    const texts = segments.slice(start, end).map(({ text }) => text)
    const context = [
      ...segments.slice(Math.max(0, start - 30), start),
      ...segments.slice(end, end + 30),
    ].map(({ text }) => text).join('\n')
    const body = { text: texts, target_lang: 'EN-US', ...(source ? { source_lang: source } : {}),
      ...(context ? { context } : {}) }
    let response: Response | undefined
    for (let attempt = 0; attempt < 3; attempt++) {
      try {
        response = await request(`${host}/v2/translate`, {
          method: 'POST', headers: { Authorization: `DeepL-Auth-Key ${apiKey}`, 'Content-Type': 'application/json' },
          body: JSON.stringify(body),
        })
      } catch {
        throw new Error('DeepL translation failed due to a network error.')
      }
      if (response.ok) break
      if (response.status === 456) throw new Error('DeepL quota exceeded (HTTP 456).')
      if (response.status !== 429 && response.status < 500) throw new Error(`DeepL translation failed (HTTP ${response.status}).`)
      if (attempt === 2) throw new Error(`DeepL translation failed after 3 attempts (HTTP ${response.status}).`)
      await wait(250 * 2 ** attempt)
    }
    let payload: unknown
    try { payload = await response!.json() } catch { throw new Error('DeepL returned invalid JSON.') }
    if (typeof payload !== 'object' || payload === null || !('translations' in payload) ||
      !Array.isArray(payload.translations) || payload.translations.length !== texts.length ||
      !payload.translations.every((item) => typeof item === 'object' && item !== null &&
        'text' in item && typeof item.text === 'string')) {
      throw new Error('DeepL returned an unexpected number or format of translations.')
    }
    payload.translations.forEach((item, index) => { result[start + index].text = item.text })
  }
  return result
}
