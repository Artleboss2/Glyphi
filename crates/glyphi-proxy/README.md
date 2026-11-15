# glyphi-proxy

The counting proxy that ships with the desktop app.

An agent points its base URL at it; every request is forwarded to the provider
while the token counts are read out of the answer.

```
OPENAI_BASE_URL=http://127.0.0.1:4787/p/openai
ANTHROPIC_BASE_URL=http://127.0.0.1:4787/p/anthropic
```

## Any endpoint works

A name is only a shortcut. In order, the proxy forwards to:

1. `?upstream=https://host/v1` or the `x-glyphi-upstream` header, when the caller
   names the endpoint itself;
2. a `providers.json` next to the binary,
   `{"my-gateway": "https://gateway.example.com/v1"}`;
3. the built-in table — 85 names, including the local runtimes (ollama, LM Studio,
   llamafile, vLLM, LocalAI, Jan, KoboldCpp, Tabby, LiteLLM) and `freebuff` /
   `codebuff` for `codebuff.com`;
4. the segment itself, when it is a host: `/p/api.example.com/v1/chat/completions`
   reaches `https://api.example.com/v1/chat/completions`, and an IP address such as
   `/p/127.0.0.1:8080/v1` is reached over plain HTTP.

So an endpoint nobody listed still works with no configuration:

```
http://127.0.0.1:4787/p/api.together.xyz/v1/chat/completions
```

## Counting

Three layers, in this order.

1. **What the provider reported.** `usage.prompt_tokens` / `completion_tokens`,
   `usage.input_tokens` / `output_tokens`, Gemini's `usageMetadata`
   (`promptTokenCount` / `candidatesTokenCount`), Ollama's `prompt_eval_count` /
   `eval_count`, Anthropic's cached-input counters, and the `usage` block nested in
   `message` or `response`.
2. **The same, inside a stream.** Each server-sent event is parsed: OpenAI's final
   `usage` chunk, Anthropic's `message_start` / `message_delta`, Gemini's
   `usageMetadata`. Measured counts are recorded as they arrive.
3. **The text itself, when nobody reports anything.** The proxy extracts the real
   prompt and the real completion — message contents, `parts`, content blocks,
   streamed deltas — and converts at 0.75 tokens per word. The JSON envelope is
   never counted, so ids, keys and model names cannot inflate the numbers. Those
   rows are marked `estimated: true`.

Each row also records **how many messages the request carried** (`messages`), and
`/__glyphi/usage` adds a `requests` count per provider per day. A chat tool that
sends no tokens at all — a local runtime, a gateway that strips `usage`, a
text-only stream — therefore still shows messages, requests and an honest estimate.

## Endpoints

| Route | What it answers |
| --- | --- |
| `GET /__glyphi/health` | `{ ok, providers }` |
| `GET /__glyphi/usage` | per-day, per-provider `{ input, output, messages, requests }` |
| `GET /__glyphi/providers` | the provider names it knows |
| `ANY /p/<provider>/<path>` | the forwarded request |

Totals are also appended to `proxy-usage.jsonl` in the app's data folder, one JSON
line per request: `{ ts, day, provider, input, output, messages, estimated }`.

## What it costs

One short-lived thread per in-flight request, one buffered body, one appended line
per request. No async runtime, no database, no OpenSSL. A release build is about
1.5 MB and an idle process sits around 6 MB of RAM. Requests keep their own
`Authorization` header: the proxy holds no credentials.

## What it cannot see

Traffic it did not route. A client that hardcodes its endpoint needs TLS
interception, which means installing a root certificate on the user's machine.
That is not worth the trust it costs.

## Build and test

```
cargo build --release
cargo test
```

`tooling/build-proxy.mjs` runs the build and stages the binary in
`apps/desktop/resources/proxy/`, which electron-forge ships as an extra resource and
the main process starts on launch.
