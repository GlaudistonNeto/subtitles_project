import env from '../../start/env.js'
import type { Segment } from '../types/subtitles.js'

function makePrompt(language: string, context: string[], texts: string[]): string {
  const data = `CONTEXT_JSON:${JSON.stringify(context)}\nINPUT_JSON:${JSON.stringify(texts)}`
  if (language === 'ja') return `音声認識の誤りを修正してください。同音異義語の漢字や助詞を文脈に基づいて直してください。前のバッチは読み取り専用の文脈です。入力と同じ数の文字列を含むJSON配列だけを返してください。\n${data}`
  if (language === 'zh') return `请根据上下文纠正语音识别错误，包括同音汉字和助词。上一批内容仅供只读参考。只返回JSON字符串数组，数量必须与输入完全相同。\n${data}`
  return `Correct speech recognition errors, including homophones and particles. The previous batch is read-only context. Return only a JSON array of strings with exactly the same number of items as the input.\n${data}`
}

export async function refineSegments(
  segments: Segment[], sourceLanguage: string, model = env.get('OLLAMA_MODEL'),
  host = env.get('OLLAMA_HOST'), request: typeof fetch = fetch,
): Promise<Segment[]> {
  const result = segments.map((segment) => ({ ...segment }))
  let context: string[] = []
  for (let start = 0; start < segments.length; start += 8) {
    const batch = segments.slice(start, start + 8)
    let refined = batch.map(({ text }) => text)
    try {
      const response = await request(`${host.replace(/\/+$/, '')}/api/chat`, {
        method: 'POST', headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ model, stream: false, format: 'json', options: { temperature: 0 },
          messages: [{ role: 'user', content: makePrompt(sourceLanguage, context, refined) }] }),
        signal: AbortSignal.timeout(60_000),
      })
      if (!response.ok) throw new Error(`Ollama returned HTTP ${response.status}`)
      const payload: unknown = await response.json()
      if (typeof payload !== 'object' || payload === null || !('message' in payload) ||
        typeof payload.message !== 'object' || payload.message === null || !('content' in payload.message) ||
        typeof payload.message.content !== 'string') throw new Error('Ollama response has no message content')
      const parsed: unknown = JSON.parse(payload.message.content)
      if (!Array.isArray(parsed) || parsed.length !== batch.length || !parsed.every((line) => typeof line === 'string')) {
        throw new Error('Ollama returned an invalid number or type of lines')
      }
      refined = parsed
    } catch (error) {
      console.warn(`Ollama refinement failed for segments ${start + 1}-${start + batch.length}; keeping original text.`, error)
    }
    batch.forEach((_, index) => { result[start + index].text = refined[index] })
    context = refined
  }
  return result
}
