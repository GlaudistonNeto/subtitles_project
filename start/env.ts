/*
|--------------------------------------------------------------------------
| Environment variables service
|--------------------------------------------------------------------------
|
| The `Env.create` method creates an instance of the Env service. The
| service validates the environment variables and also cast values
| to JavaScript data types.
|
*/

import { Env } from '@adonisjs/core/env'

export default await Env.create(new URL('../', import.meta.url), {
  NODE_ENV: Env.schema.enum(['development', 'production', 'test'] as const),
  PORT: Env.schema.number(),
  APP_KEY: Env.schema.string(),
  HOST: Env.schema.string({ format: 'host' }),
  LOG_LEVEL: Env.schema.string(),
  SESSION_DRIVER: Env.schema.enum(['cookie', 'memory'] as const),
  DEEPL_API_KEY: Env.schema.string(),
  OLLAMA_HOST: (key, value) =>
    Env.schema.string.optional()(key, value) ?? 'http://localhost:11434',
  OLLAMA_MODEL: (key, value) =>
    Env.schema.string.optional()(key, value) ?? 'qwen2.5:14b',
  WHISPER_MODEL: (key, value) =>
    Env.schema.string.optional()(key, value) ?? 'large-v3',
  MAX_UPLOAD_MB: (key, value) => Env.schema.number.optional()(key, value) ?? 500,
})
