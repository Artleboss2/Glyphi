# glyphi-proxy

The counting proxy that ships with the desktop app.

An agent points its base URL at it; every request is forwarded to the provider
while the token counts are read out of the answer.

```
OPENAI_BASE_URL=http://127.0.0.1:4787/p/openai
ANTHROPIC_BASE_URL=http://127.0.0.1:4787/p/anthropic
```

83 providers are known out of the box (including the local ones: ollama, LM Studio,
llamafile, vLLM, LocalAI, Jan, KoboldCpp, Tabby, LiteLLM). Any other endpoint works
with an explicit upstream, or by adding it to a `providers.json` next to the binary:

```
http://127.0.0.1:4787/p/my-gateway/v1/chat/completions?upstream=https://gateway.example.com/v1
```

## What it costs

One short-lived thread per in-flight request, one buffered body, one appended line
per request. No async runtime, no database, no OpenSSL. A release build is about
1.5 MB and an idle process sits around 6 MB of RAM. Requests keep their own
`Authorization` header: the proxy holds no credentials.

## Endpoints

| Route | What it answers |
| --- | --- |
| `GET /__glyphi/health` | `{ ok, providers }` |
| `GET /__glyphi/usage` | per-day, per-provider totals |
| `GET /__glyphi/providers` | the provider names it knows |
| `ANY /p/<provider>/<path>` | the forwarded request |

Totals are also appended to `proxy-usage.jsonl` in the app's data folder, one JSON
line per request: `{ ts, day, provider, input, output, estimated }`.

## Counting

The provider's own numbers win: `usage.prompt_tokens` / `completion_tokens`,
`usage.input_tokens` / `output_tokens`, plus `cache_read_input_tokens`, including
inside server-sent event streams. When a provider reports nothing, the proxy counts
words and converts at the usual 0.75 tokens per word, and marks the row
`estimated: true` so a dashboard can say which numbers are measured.

## What it cannot see

Traffic it did not route. A client that hardcodes its endpoint (the Freebuff desktop
app talks only to `codebuff.com`) would need TLS interception, which means installing
a root certificate on the user's machine. That is not worth the trust it costs.

## Build

```
cargo build --release
```

`tooling/build-proxy.mjs` runs this and stages the binary in
`apps/desktop/resources/proxy/`, which electron-forge ships as an extra resource and
the main process starts on launch.
