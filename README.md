# subtitles_project

Gera legendas SRT em inglês a partir de vídeos ou áudios falados em japonês, chinês ou outros idiomas.
Transcrição e refino são locais; só a tradução usa a API do DeepL.

Pipeline: FFmpeg → Whisper large-v3 (GPU/CUDA, com VAD) → Ollama (corrige erros de reconhecimento) → DeepL (EN-US) → `.srt`

## Requisitos
- Node.js 22+, pnpm, FFmpeg, CMake e um compilador C++
- Ollama com o modelo: `ollama pull qwen2.5:14b`
- Chave de API do DeepL (plano gratuito serve)
- Opcional: GPU NVIDIA com CUDA Toolkit 13.x (sem GPU, roda em CPU, mais devagar)

## Instalação
```bash
pnpm install
NODEJS_WHISPER_CUDA=1 pnpm rebuild nodejs-whisper   # compila o Whisper com CUDA (demora)
cp .env.example .env
node ace generate:key
# edite o .env e preencha DEEPL_API_KEY
```
O modelo `large-v3` (~3 GB) é baixado na primeira execução para `tmp/whisper-models/`.

## Uso
```bash
node ace subtitles:generate video.mp4 --language ja
```
Opções: `--language ja|zh|auto`, `--skip-refine`, `--output caminho.srt`, `--ollama-model`, `--whisper-model`, `--keep-audio`.
O resultado vai para `<video>.en.srt`.

Interface web: `pnpm dev` e abra http://localhost:3333 (um processamento por vez).

## Testes
```bash
pnpm typecheck
pnpm test
```

## Como os tempos são tratados
Cada legenda usa o início e o fim reais do segmento falado. O VAD do Whisper impede que a primeira legenda comece em 0 s quando o áudio tem silêncio no início. O refino e a tradução só alteram o texto, nunca os tempos. Textos longos viram várias legendas de até 2 linhas de 42 caracteres.

## Limitações
O Whisper às vezes inventa uma frase curta depois do fim da fala. A chave do DeepL fica só no `.env`, que não vai para o Git.

## Licença
MIT
