Estrutura de Prompt (subtitles_project: AdonisJS + TypeScript + pnpm + Japa)

1. Contexto da tarefa

Você é um(a) dev fullstack sênior em Node.js/TypeScript com experiência em AdonisJS. Sua missão é CRIAR DO ZERO um projeto chamado **subtitles_project**, usando apenas este prompt como especificação. Não existe código anterior para ler, clonar ou migrar: o diretório atual contém somente este `prompt.md`.

O projeto gera legendas SRT em inglês (EN-US) a partir de vídeos falados em japonês, chinês ou outros idiomas, com transcrição e refino 100% locais:

Vídeo → FFmpeg (WAV 16 kHz mono + passa-alta + loudnorm) → Whisper large-v3 (whisper.cpp via `nodejs-whisper`, com CUDA quando disponível) → Ollama (corrige erros de ASR) → DeepL (EN-US, em lotes) → .srt

Entregáveis:

1. Aplicação AdonisJS (TypeScript, pnpm) com o pipeline implementado em serviços reutilizáveis.
2. Dois pontos de entrada para o MESMO pipeline:
   - Comando Ace: `node ace subtitles:generate <video>` (principal).
   - Interface web mínima: uma página com formulário para enviar o vídeo, escolher opções e baixar o SRT.
3. Testes automatizados com Japa (runner oficial do AdonisJS).
4. Validação do serviço web com o Playwright MCP.
5. README.md em português com instruções claras para instalar, rodar e testar.
6. LICENSE (MIT).

Sem banco de dados, sem Lucid, sem autenticação. Os arquivos enviados e gerados ficam em um diretório temporário (`tmp/`), fora do versionamento.

2. Contexto de tom

Direto, didático e enxuto. Explique só o essencial para instalar, rodar e testar localmente. Responda em português.

3. Dados de antecedentes, documentos e MCPs

Você TEM acesso a MCPs no VS Code e DEVE usar o Context7 MCP e o Playwright MCP. Regra crítica:

Se o Context7 MCP não estiver disponível/funcionando, PARE o processo imediatamente e responda apenas: “Context7 MCP não disponível. Não posso continuar.”

Se o Playwright MCP não estiver disponível, continue a geração, mas avise na saída final que a validação web não pôde ser feita.

Regras de consulta (Context7):

Use o Context7 para buscar a documentação ATUAL (não confie na memória) sobre:

- **AdonisJS**: criação do projeto (kit `web` com Edge, sem Lucid/Auth/sessão se possível), requisitos de versão do Node.js, comandos Ace customizados (`make:command`, `@args`, `@flags`, `run()`, `this.logger`), controllers, rotas, upload multipart (`request.file`, limites de tamanho e extensões), validação de env em `start/env.ts`, serviços com injeção de dependência e aliases de import (`#services/*`).
- **nodejs-whisper**: instalação, compilação do whisper.cpp, download do modelo `large-v3` e `large-v3-turbo`, opções de idioma, formato de saída com TIMESTAMPS POR SEGMENTO e suporte a CUDA (flag/opção atual de build e de execução).
- **DeepL**: biblioteca/API oficial, tradução para EN-US, parâmetro `context`, limites do plano Free, erros e rate limit.
- **Ollama**: API HTTP local (`/api/chat`), formato das respostas, temperatura, e como listar/baixar modelos.
- **Japa**: configuração em `adonisrc.ts` e `tests/bootstrap.ts`, suites `unit`/`functional`, `@japa/api-client`, `node ace test`, e como substituir dependências nos testes (`app.container.swap`) ou mockar `fetch` e processos filhos.
- **FFmpeg** (linha de comando): filtros `highpass` e `loudnorm`, `-ar 16000 -ac 1`.

Antes do código, mostre:

“Docs consultados:” + títulos das páginas

até 8–10 linhas totais de snippets (curtos) usados como base

4. Descrição detalhada da tarefa e regras

Gere o código e os arquivos mínimos para o projeto funcionar e ser testado, sem passos desnecessários.

Requisitos técnicos:

- AdonisJS (versão estável atual) + TypeScript.
- Gerenciador: **pnpm** (obrigatório). Não use npm nem yarn. Se o pnpm pedir aprovação de scripts de build (ex.: `nodejs-whisper`), configure `onlyBuiltDependencies` ou rode `pnpm approve-builds`/`pnpm rebuild nodejs-whisper`, conforme a doc atual.
- Dependências: liste e instale apenas o necessário.
- Código compatível com Windows, macOS e Linux (use `node:path`, nunca concatene caminhos com `/` ou `\`).
- Se o diretório atual já tiver arquivos além do `prompt.md`, não os apague; crie o projeto de forma que o `prompt.md` permaneça na raiz.

Arquitetura (confirme nomes e padrões no Context7):

```
commands/subtitles_generate.ts          Comando Ace: node ace subtitles:generate
app/controllers/subtitles_controller.ts Página + upload + download do SRT
app/services/audio_service.ts           Extração de áudio (FFmpeg)
app/services/transcription_service.ts   Transcrição (nodejs-whisper), retorna segmentos com tempo
app/services/refinement_service.ts      Refino com Ollama
app/services/translation_service.ts     Tradução DeepL
app/services/srt_service.ts             Quebra de linhas e geração do SRT
app/services/subtitle_pipeline.ts       Orquestra as etapas
app/types/subtitles.ts                  Tipos (Segment, PipelineOptions, etc.)
resources/views/pages/home.edge         Formulário (Tailwind via CDN ou CSS simples)
start/routes.ts                         GET / , POST /subtitles
start/env.ts                            Validação das variáveis de ambiente
tests/unit/                             Testes unitários por serviço
tests/functional/                       Testes do comando Ace e das rotas
tests/fixtures/                         Áudio curto (ver "Fixture")
```

Opções do pipeline (as mesmas no comando Ace e no formulário web):

| Opção              | Descrição                                                             |
|--------------------|-----------------------------------------------------------------------|
| `<video>`          | Argumento obrigatório no Ace; campo de upload na web                  |
| `--language`, `-l` | `ja`, `zh` ou `auto` (padrão)                                         |
| `--skip-refine`    | Pula a correção com Ollama                                            |
| `--output`, `-o`   | Caminho do SRT (padrão: `<video>.en.srt`); na web, download do arquivo |
| `--ollama-model`   | Modelo Ollama (padrão `qwen2.5:7b`; alternativa `qwen2.5:14b`)        |
| `--whisper-model`  | Modelo Whisper (padrão `large-v3`; mais rápido: `large-v3-turbo`)     |
| `--keep-audio`     | Mantém o WAV temporário                                               |

Regras do pipeline:

- **Áudio**: WAV 16 kHz mono, passa-alta de 80 Hz e `loudnorm`. NÃO use filtros que removam ou encurtem silêncio (`silenceremove`, `atrim`, `-ss` etc.): eles deslocam a linha do tempo e quebram a sincronização das legendas.
- **Transcrição**: Whisper `large-v3`, idioma forçado quando `--language` for `ja`/`zh` (evita detecção errada); detecção automática quando `auto`. O serviço deve devolver uma lista de segmentos `{ index, startMs, endMs, text }`.
- **Refino (Ollama)**: lotes de 8 segmentos, com contexto do lote anterior; prompts escritos no idioma de origem (japonês/chinês), para corrigir kanji/hanzi homófonos e partículas. Só o texto é alterado, nunca o tempo nem a quantidade de segmentos. Se o Ollama falhar (indisponível, timeout, resposta inválida), mantém a transcrição original daquele lote, sem lançar erro.
- **Tradução (DeepL)**: EN-US, em lotes de 30, enviando o texto vizinho no parâmetro `context` para recuperar sujeitos omitidos (comuns em japonês/chinês). Só o texto é alterado.
- **SRT**: no máximo 42 caracteres × 2 linhas; duração mínima de 1 s; quebra de linha de CJK por contagem de caracteres e pontuação (、。，！？), nunca por espaços; índices sequenciais; timestamps no formato `HH:MM:SS,mmm`.
- Limpe o WAV temporário ao final, exceto com `--keep-audio`, inclusive em caso de erro.

Sincronização de tempo (regra crítica):

- Cada legenda usa os timestamps reais de início/fim do segmento falado, vindos do Whisper. A primeira legenda NUNCA começa em 00:00:00,000 só porque o vídeo começou; se há silêncio ou música antes da primeira fala, a legenda começa quando a fala começa.
- Os tempos de cada segmento (`startMs`, `endMs`) são preservados intactos por refino, tradução e quebra de linhas. A regra de duração mínima de 1 s só pode estender o FIM de uma legenda (sem ultrapassar o início da próxima); nunca antecipa o início.
- Confirme no Context7 como obter timestamps por segmento no `nodejs-whisper` (formato de saída) e faça o parsing correto, incluindo a conversão para milissegundos.

Aceleração por GPU (CUDA):

- Configure o `nodejs-whisper` para usar CUDA quando houver GPU NVIDIA. O ambiente de referência usa o CUDA Toolkit **13.5**; confirme no Context7 a flag/opção atual de build e execução e a versão mínima exigida, e verifique a versão instalada com `nvcc --version`, sem assumir. Se a versão instalada diferir, avise e prossiga.
- Se não houver GPU/CUDA, o pipeline cai para CPU com um aviso no log e NÃO falha.
- A decisão GPU/CPU fica isolada em uma função pura (ex.: montagem das opções de transcrição), para ser testável sem GPU.
- Documente no README os pré-requisitos (driver NVIDIA, CUDA Toolkit, cmake e ferramentas de build) e o comando de rebuild do `nodejs-whisper` com CUDA.

Interface web (mínima, para o Playwright validar):

- `GET /`: página com um formulário (campo de arquivo `video`, seleção de idioma `auto/ja/zh`, checkbox `skip-refine`, seleção do modelo Whisper, botão “Gerar legenda”), título e textos em português.
- `POST /subtitles`: valida o arquivo (extensões de vídeo/áudio comuns e tamanho máximo configurável por env), executa o pipeline e devolve o `.srt` como download. Entrada inválida retorna `400` com mensagem clara; falha de pipeline retorna `500` com mensagem sem dados sensíveis.
- A página mostra um estado “Processando…” durante o envio e uma mensagem de erro legível em caso de falha. Use `data-testid` nos elementos principais (formulário, campo de arquivo, botão, mensagem de erro) para facilitar o Playwright.
- O processamento é síncrono e simples; não implemente filas nem jobs.

Configuração e segurança:

- Variáveis de ambiente validadas em `start/env.ts`: `DEEPL_API_KEY` (obrigatória para executar o pipeline completo), `OLLAMA_HOST` (padrão `http://localhost:11434`), `OLLAMA_MODEL`, `WHISPER_MODEL`, `MAX_UPLOAD_MB`, além das variáveis padrão do Adonis (`PORT`, `APP_KEY` etc.).
- Gere `.env.example` apenas com placeholders (ex.: `DEEPL_API_KEY=`). NUNCA coloque valores reais, não leia o `.env` em voz alta, não imprima chaves em logs nem em mensagens de erro.
- `.gitignore` deve incluir `.env`, `tmp/`, `node_modules/`, modelos do Whisper baixados e a pasta de build do whisper.cpp.
- Os serviços recebem suas dependências externas (FFmpeg, Whisper, Ollama, DeepL) por injeção, para permitir substituição nos testes.

Testes automatizados (Japa, via `node ace test`):

- Testes unitários SEM rede, SEM GPU e SEM binários pesados: mocke FFmpeg, Whisper, Ollama e DeepL.
- Cubra, no mínimo:
  - `srt_service`: quebra de linhas CJK, limite de 42 caracteres × 2 linhas, duração mínima de 1 s só estendendo o fim, índices sequenciais e formato de timestamps.
  - Sincronização: com segmentos simulados que começam após N segundos de silêncio, a primeira legenda do SRT começa em N s; início/fim de cada segmento permanecem iguais após refino e tradução.
  - `refinement_service`: lotes de 8, uso do contexto do lote anterior e fallback ao texto original quando o Ollama falha.
  - `translation_service`: lotes de 30, envio do parâmetro `context`, tratamento de erro/rate limit.
  - `audio_service`: argumentos corretos do FFmpeg (incluindo ausência de filtros que removam silêncio) e remoção do WAV temporário.
  - `transcription_service`: escolha GPU/CPU (função pura) e parsing de segmentos com tempo.
  - Comando Ace: parsing de argumentos/flags, valores padrão e saída `<video>.en.srt`.
  - Rotas (`@japa/api-client`, pipeline mockado via `app.container.swap`): `GET /` retorna 200; `POST /subtitles` sem arquivo retorna 400; com arquivo válido devolve SRT.
- Teste de integração em suite separada (`integration`), fora do `pnpm test` padrão: roda o pipeline real com a fixture e é PULADO automaticamente se faltarem FFmpeg, o modelo Whisper ou `DEEPL_API_KEY`.
- Scripts no `package.json`: `dev`, `build`, `start`, `test` (unit + functional), `test:integration` e `typecheck`.

Fixture:

- Para o teste de integração e a validação ponta a ponta, é necessário um áudio curto (10–20 s) em `tests/fixtures/` com alguns segundos de SILÊNCIO inicial seguidos de fala em japonês ou chinês.
- Se o arquivo não existir, tente gerá-lo (ex.: síntese de voz local, como `espeak-ng -v ja`, concatenada com silêncio via FFmpeg). Se não for possível, crie `tests/fixtures/README.md` explicando o que colocar ali, PULE a validação ponta a ponta e informe isso claramente na saída. Não invente resultados.

Validação (Playwright MCP):

- Depois de gerar os arquivos, rode `pnpm install`, `pnpm typecheck` e `pnpm test` e mostre o resultado.
- Inicie o projeto (`pnpm dev` ou equivalente) e valide com o Playwright MCP que o serviço está funcionando na porta correta (padrão do Adonis: 3333, ou o valor de `PORT`):
  1. A página `/` carrega e exibe o formulário.
  2. Enviar o formulário sem arquivo mostra a mensagem de erro de validação.
  3. Se a fixture e todos os pré-requisitos (FFmpeg, modelo Whisper, Ollama ou `skip-refine`, `DEEPL_API_KEY`) existirem: enviar a fixture com idioma `ja`, aguardar o download do `.srt` e conferir que é válido (índices sequenciais, timestamps crescentes, linhas dentro do limite) e que a primeira legenda NÃO começa em `00:00:00,000`.
- Verifique o CUDA: mostre `nvcc --version` e `nvidia-smi` (se existirem) e a evidência no log da transcrição de uso da GPU ou o aviso de fallback para CPU.
- Se algum pré-requisito do sistema faltar (FFmpeg, cmake, Ollama, chave DeepL, GPU), diga exatamente qual e execute o que for possível; não invente resultados.

Comportamento esperado:

- `node ace subtitles:generate video.mp4 --language ja` gera `video.en.srt`.
- A interface web em `http://localhost:3333` envia um vídeo e devolve o SRT para download.
- `--skip-refine` não faz nenhuma chamada ao Ollama.
- Falha no Ollama não interrompe o pipeline.
- A primeira legenda começa quando a fala começa, não quando o vídeo começa.
- Todos os testes unitários passam sem internet e sem GPU.

8. Pensar passo a passo / respirar fundo

Pense passo a passo internamente para evitar erros de caminhos/exports/imports, aliases (`#services/*`, `#types/*`) e configuração de ESM do Adonis. NÃO mostre seu raciocínio. Mostre apenas o resultado final.

9. Formatação da saída

Responda em português e siga EXATAMENTE esta ordem:

1. Verificação do Context7 (1 linha: “Context7 OK” ou a mensagem de parada)
2. Docs consultados (títulos + snippets curtos)
3. Dependências (lista curta, separando `dependencies` e `devDependencies`)
4. Estrutura de arquivos criados (árvore)
5. Comandos pnpm (na ordem: instalar dependências, rebuild do `nodejs-whisper` com CUDA, baixar modelos Ollama, rodar testes, rodar o projeto)
6. Resultado da validação (saída de `pnpm test`, validação com o Playwright MCP e evidência de CUDA/tempo, ou o motivo de não ter sido possível)
