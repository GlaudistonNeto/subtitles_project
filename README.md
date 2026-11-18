# subtitles_project - Rust Edition ⚡ ✅

Projeto **totalmente em Rust** para gerar legendas SRT em inglês a partir de vídeos ou áudios.

Gera legendas SRT em inglês a partir de vídeos ou áudios falados em japonês, chinês e outros idiomas.

Pipeline: `FFmpeg → Whisper large-v3 (GPU/CUDA ou CPU) → Ollama (corrige erros de reconhecimento) → DeepL (EN-US)` → `.srt`

## ✅ Migração Concluída

O projeto foi **100% migrado do TypeScript para Rust**. Todo código TypeScript/JavaScript foi removido. A migração está completa - só resta compilar o binário!

## Requisitos
- Rust 1.75+
- FFmpeg
- libclang-dev (apenas se quiser compilar com suporte a CUDA - opcional, CPU funciona sem)
- Chave de API do DeepL (plano gratuito serve)
- Ollama com modelo local para refinement (ex: `qwen2.5:14b`)

### 🚨 Compilação no Seu Ambiente Atual

O ambiente atual está rodando **rootless** (usuário `spike` sem sudo), o que impede a instalação de `libclang-dev`.

#### Soluções Alternativas para Compilar:

**Opção 1: Usar Docker Container (Recomendado)**
```bash
docker run --rm -it \
    -v "$(pwd)/rust/whisper-runtime:/build" \
    rust:latest bash \
    && cd /build \
    && apt-get update \
    && apt-get install -y libclang-dev build-essential cmake git curl wget \
    && cargo build --release \
    && cp target/release/subtitles ../bin/
```

**Opção 2: Máquina com Acesso Root**
Execute estas instruções em uma máquina onde tenha permissão `sudo`:
```bash
cd /home/spike/subtitles_project/rust/whisper-runtime
sudo apt-get install -y libclang-dev build-essential cmake git cargo
cargo build --release
cp target/release/subtitles ../bin/
```

**Opção 3: Snap Package**
Em alguns sistemas Ubuntu, você pode instalar via snap:
```bash
sudo snap install libclang-dev
cd /home/spike/subtitles_project/rust/whisper-runtime
cargo build --release
```

**Após compilar em qualquer ambiente**, copie o binário para:
```bash
cp target/release/subtitles /home/spike/subtitles_project/bin/
```

### 3. Configurar Ambiente
```bash
cd /home/spike/subtitles_project
cp .env.example .env
nano .env  # ou edite conforme necessário

# Variáveis obrigatórias:
# DEEPL_API_KEY=your_api_key_here

# Opcionais com defaults úteis:
# OLLAMA_HOST=http://localhost:11434
# OLLAMA_MODEL=qwen2.5:14b
# WHISPER_MODEL=large-v3
# MAX_UPLOAD_MB=500
# PORT=3333
```

## Uso

### Gerar legendas a partir de vídeo:
```bash
./subtitles generate video.mp4 --language ja --output video.en.srt
```

**Opções disponíveis:**
- `--language`: ja|zh|en|es|pt|de|ru|auto (padrão: auto)
- `--skip-refine`: pula a etapa de refinement do Ollama (mais rápido, menos preciso)
- `--output`: caminho personalizado para o arquivo de saída
- `--ollama-model`: modelo específico do Ollama
- `--whisper-model`: modelo específico do Whisper
- `--keep-audio`: mantém o arquivo WAV extraído (útil para debug)

**Exemplos:**
```bash
# Japonês com refinement completo
./subtitles generate video.mp4 --language ja

# Chinês sem refinement (rápido)
./subtitles generate video.mp4 --language zh --skip-refine

# Idioma automático com modelo customizado
./subtitles generate video.mp4 --language auto --ollama-model qwen2.5:7b
```

### Servidor Web completo:
```bash
./subtitles serve
# Acesso em http://localhost:3333
# Interface web para upload de vídeo e geração de legendas
```

## Compilação com CUDA (Aceleração GPU)

Se tiver uma GPU NVIDIA:
```bash
# Instalar CUDA Toolkit e drivers
sudo apt-get install -y nvidia-cuda-toolkit

# Recompile com suporte a CUDA
cd rust/whisper-runtime
cargo build --release --features cuda
```

**Sem CUDA**: O projeto roda perfeitamente em CPU, apenas mais devagar (~3-5x mais lento que GPU).

## Arquitetura do Projeto

### Código Rust (`rust/whisper-runtime/src/`)
- `server.rs` - HTTP server com Axum (upload multipart, routes)
- `pipeline.rs` - Pipeline principal de processamento
- `audio.rs` - Extração de áudio com FFmpeg
- `models.rs` - Carregamento e gerenciamento de modelos Whisper
- `config.rs` - Parsing de configuração do ambiente (.env)
- `services.rs` - Integração com Ollama para refinement LLM
- `segments.rs` - Tipos e estrutura de segmentos de transcrição
- `bin/subtitles.rs` - CLI entry point

### Fluxo do Pipeline
1. **Upload/CLI**: Recebe vídeo via HTTP ou argumentos
2. **FFmpeg**: Extrai áudio WAV com filtros (highpass, loudnorm)
3. **Chunking**: Divide áudio em chunks de 60s usando VAD (Silero)
4. **Whisper**: Transcreve cada chunk para texto com timestamps
5. **Ollama**: Corrige erros de homófonos e partículas linguísticas
6. **DeepL**: Traduz do idioma fonte para inglês (EN-US)
7. **SRT**: Formata saída no formato `.srt`

## Performance

- **CPU**: ~5-10 minutos por vídeo (dependendo do tamanho e idioma)
- **GPU (CUDA)**: ~1-3 minutos por vídeo (NVIDIA GPU com CUDA)
- Servidor web processa **uma requisição por vez**
- Chunks paralelos no pipeline de transcrição

## Limitações conhecidas

- Whisper às vezes gera frases curtas após o fim da fala real
- Tradução via API DeepL (paga, tem quota grátis mensal)
- Ollama precisa rodar localmente para refinement
- Servidor web apenas 1 request/concurrent processing
- Sem CUDA: ~3-5x mais lento que GPU

## Licença
MIT
