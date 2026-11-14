//! Glyphi's counting proxy.
//!
//! One process, no async runtime, no database. An agent points its base URL at
//! `http://127.0.0.1:4787/p/<provider>` and every request is forwarded to that
//! provider's real endpoint while the token counts are read out of the response
//! (or, when a provider reports nothing, out of the text itself).
//!
//! Memory: one short-lived thread per in-flight request, one buffered body, and
//! an append-only line per request. Nothing is kept in memory between requests,
//! so the resident size of an idle proxy is a couple of megabytes.
//!
//! What it cannot do: read HTTPS traffic it did not route. A client that only
//! talks to `codebuff.com` and offers no base-URL setting (the Freebuff desktop
//! app) needs TLS interception, which means shipping a root certificate — out of
//! scope on purpose.

use std::collections::BTreeMap;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Map, Value};

const DEFAULT_PORT: u16 = 4787;
const MAX_BODY: usize = 32 * 1024 * 1024;

/// Providers that take an OpenAI-compatible base URL. The prefix after `/p/` is
/// the key; a name that is not listed is still forwarded when the caller passes
/// the upstream as `?upstream=https://host/v1`.
const PROVIDERS: &[(&str, &str)] = &[
    ("openai", "https://api.openai.com/v1"),
    ("anthropic", "https://api.anthropic.com/v1"),
    ("google", "https://generativelanguage.googleapis.com/v1beta"),
    ("gemini", "https://generativelanguage.googleapis.com/v1beta"),
    ("vertex", "https://us-central1-aiplatform.googleapis.com/v1"),
    ("azure", "https://YOUR-RESOURCE.openai.azure.com/openai"),
    ("mistral", "https://api.mistral.ai/v1"),
    ("groq", "https://api.groq.com/openai/v1"),
    ("deepseek", "https://api.deepseek.com/v1"),
    ("xai", "https://api.x.ai/v1"),
    ("together", "https://api.together.xyz/v1"),
    ("fireworks", "https://api.fireworks.ai/inference/v1"),
    ("perplexity", "https://api.perplexity.ai"),
    ("openrouter", "https://openrouter.ai/api/v1"),
    ("cohere", "https://api.cohere.com/v1"),
    ("cerebras", "https://api.cerebras.ai/v1"),
    ("sambanova", "https://api.sambanova.ai/v1"),
    ("nebius", "https://api.studio.nebius.ai/v1"),
    ("hyperbolic", "https://api.hyperbolic.xyz/v1"),
    ("novita", "https://api.novita.ai/v3/openai"),
    ("lambda", "https://api.lambdalabs.com/v1"),
    ("replicate", "https://api.replicate.com/v1"),
    ("huggingface", "https://api-inference.huggingface.co"),
    ("nvidia", "https://integrate.api.nvidia.com/v1"),
    ("github", "https://models.inference.ai.azure.com"),
    ("ollama", "http://127.0.0.1:11434/v1"),
    ("lmstudio", "http://127.0.0.1:1234/v1"),
    ("llamafile", "http://127.0.0.1:8080/v1"),
    ("vllm", "http://127.0.0.1:8000/v1"),
    ("localai", "http://127.0.0.1:8080/v1"),
    ("jan", "http://127.0.0.1:1337/v1"),
    ("gpt4all", "http://127.0.0.1:4891/v1"),
    ("textgenwebui", "http://127.0.0.1:5000/v1"),
    ("koboldcpp", "http://127.0.0.1:5001/v1"),
    ("tabby", "http://127.0.0.1:8080/v1"),
    ("gitea", "http://127.0.0.1:3000/api/v1"),
    ("reka", "https://api.reka.ai/v1"),
    ("ai21", "https://api.ai21.com/studio/v1"),
    ("writer", "https://api.writer.com/v1"),
    ("voyage", "https://api.voyageai.com/v1"),
    ("jina", "https://api.jina.ai/v1"),
    ("voyager", "https://api.voyageai.com/v1"),
    ("deepinfra", "https://api.deepinfra.com/v1/openai"),
    ("featherless", "https://api.featherless.ai/v1"),
    ("chutes", "https://llm.chutes.ai/v1"),
    ("kluster", "https://api.kluster.ai/v1"),
    ("atoma", "https://api.atoma.network/v1"),
    ("galadriel", "https://api.galadriel.com/v1"),
    ("anyscale", "https://api.endpoints.anyscale.com/v1"),
    ("octoai", "https://text.octoai.run/v1"),
    ("lepton", "https://<model>.lepton.run/api/v1"),
    ("predibase", "https://serving.app.predibase.com"),
    ("cloudflare", "https://api.cloudflare.com/client/v4"),
    ("qwen", "https://dashscope-intl.aliyuncs.com/compatible-mode/v1"),
    ("dashscope", "https://dashscope-intl.aliyuncs.com/compatible-mode/v1"),
    ("moonshot", "https://api.moonshot.cn/v1"),
    ("zhipu", "https://open.bigmodel.cn/api/paas/v4"),
    ("baichuan", "https://api.baichuan-ai.com/v1"),
    ("minimax", "https://api.minimax.chat/v1"),
    ("stepfun", "https://api.stepfun.com/v1"),
    ("lingyi", "https://api.lingyiwanwu.com/v1"),
    ("siliconflow", "https://api.siliconflow.cn/v1"),
    ("volcengine", "https://ark.cn-beijing.volces.com/api/v3"),
    ("upstage", "https://api.upstage.ai/v1/solar"),
    ("nlpcloud", "https://api.nlpcloud.io/v1"),
    ("eden", "https://api.edenai.run/v2"),
    ("stability", "https://api.stability.ai/v1"),
    ("elevenlabs", "https://api.elevenlabs.io/v1"),
    ("assemblyai", "https://api.assemblyai.com/v2"),
    ("deepgram", "https://api.deepgram.com/v1"),
    ("pinecone", "https://api.pinecone.io"),
    ("weaviate", "http://127.0.0.1:8080/v1"),
    ("chroma", "http://127.0.0.1:8000/api/v1"),
    ("qdrant", "http://127.0.0.1:6333"),
    ("langsmith", "https://api.smith.langchain.com"),
    ("litellm", "http://127.0.0.1:4000/v1"),
    ("portkey", "https://api.portkey.ai/v1"),
    ("helicone", "https://oai.helicone.ai/v1"),
    ("unify", "https://api.unify.ai/v0"),
    ("aimlapi", "https://api.aimlapi.com/v1"),
    ("requesty", "https://router.requesty.ai/v1"),
    ("cometapi", "https://api.cometapi.com/v1"),
    ("openai-compatible", "http://127.0.0.1:8080/v1")
];

fn providers() -> BTreeMap<String, String> {
    let mut map: BTreeMap<String, String> = PROVIDERS
        .iter()
        .map(|(name, url)| ((*name).to_string(), (*url).to_string()))
        .collect();

    // A `providers.json` next to the executable (or in the working directory)
    // adds or overrides entries: {"my-gateway": "https://gateway.example.com/v1"}.
    for candidate in config_paths() {
        let Ok(text) = fs::read_to_string(&candidate) else { continue };
        let Ok(Value::Object(entries)) = serde_json::from_str::<Value>(&text) else { continue };
        for (name, value) in entries {
            if let Value::String(url) = value {
                map.insert(name.to_lowercase(), url);
            }
        }
        eprintln!("glyphi-proxy: loaded providers from {}", candidate.display());
    }
    map
}

fn config_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(exe) = env::current_exe() {
        if let Some(directory) = exe.parent() {
            paths.push(directory.join("providers.json"));
        }
    }
    paths.push(PathBuf::from("providers.json"));
    paths
}

fn usage_log() -> PathBuf {
    if let Ok(path) = env::var("GLYPHI_PROXY_LOG") {
        return PathBuf::from(path);
    }
    let base = env::var("APPDATA")
        .or_else(|_| env::var("XDG_DATA_HOME"))
        .unwrap_or_else(|_| ".".to_string());
    let directory = PathBuf::from(base).join("Glyphi Teams");
    let _ = fs::create_dir_all(&directory);
    directory.join("proxy-usage.jsonl")
}

fn main() {
    let port: u16 = env::var("GLYPHI_PROXY_PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_PORT);
    let providers = providers();
    let log = usage_log();
    eprintln!(
        "glyphi-proxy: {} providers, listening on http://127.0.0.1:{port}/p/<provider>, log {}",
        providers.len(),
        log.display()
    );

    let listener = match TcpListener::bind(("127.0.0.1", port)) {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("glyphi-proxy: cannot bind 127.0.0.1:{port}: {error}");
            std::process::exit(1);
        }
    };

    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let providers = providers.clone();
        let log = log.clone();
        thread::spawn(move || {
            if let Err(error) = handle(stream, &providers, &log) {
                eprintln!("glyphi-proxy: {error}");
            }
        });
    }
}

fn handle(mut stream: TcpStream, providers: &BTreeMap<String, String>, log: &PathBuf) -> std::io::Result<()> {
    stream.set_nodelay(true)?;
    let Some(request) = read_request(&mut stream)? else { return Ok(()) };

    if request.path == "/__glyphi/health" {
        return respond_json(&mut stream, 200, &json!({ "ok": true, "providers": providers.len() }));
    }
    if request.path == "/__glyphi/usage" {
        return respond_json(&mut stream, 200, &aggregate(log));
    }
    if request.path == "/__glyphi/providers" {
        let list: Vec<String> = providers.keys().cloned().collect();
        return respond_json(&mut stream, 200, &json!(list));
    }

    let Some(rest) = request.path.strip_prefix("/p/") else {
        return respond_json(
            &mut stream,
            404,
            &json!({ "error": "Use /p/<provider>/<path>, or /__glyphi/usage" })
        );
    };
    let (provider, tail) = match rest.split_once('/') {
        Some((provider, tail)) => (provider, format!("/{tail}")),
        None => (rest, String::from("/")),
    };

    let override_upstream = query_param(&request.path, "upstream");
    let upstream_base = override_upstream
        .or_else(|| providers.get(&provider.to_lowercase()).cloned());
    let Some(base) = upstream_base else {
        return respond_json(
            &mut stream,
            404,
            &json!({ "error": format!("Unknown provider '{provider}'. Add it to providers.json or pass ?upstream=.") })
        );
    };
    let url = format!("{}{}", base.trim_end_matches('/'), strip_query(&tail));

    let response = forward(&request, &url);
    let (status, headers, body) = match response {
        Ok(parts) => parts,
        Err(error) => {
            return respond_json(&mut stream, 502, &json!({ "error": error }));
        }
    };

    // Count before answering, so a client that disconnects still leaves a record.
    let counts = count_tokens(&request.body, &body, header_value(&headers, "content-type").as_deref());
    record(log, provider, &counts);

    write_response(&mut stream, status, &headers, &body)
}

struct Request {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

fn read_request(stream: &mut TcpStream) -> std::io::Result<Option<Request>> {
    let mut buffer = Vec::with_capacity(8192);
    let mut chunk = [0u8; 8192];
    let header_end;
    loop {
        let read = stream.read(&mut chunk)?;
        if read == 0 {
            return Ok(None);
        }
        buffer.extend_from_slice(&chunk[..read]);
        if let Some(position) = find(&buffer, b"\r\n\r\n") {
            header_end = position + 4;
            break;
        }
        if buffer.len() > 64 * 1024 {
            return Ok(None);
        }
    }

    let head = String::from_utf8_lossy(&buffer[..header_end]).to_string();
    let mut lines = head.split("\r\n");
    let mut request_line = lines.next().unwrap_or_default().split_whitespace();
    let method = request_line.next().unwrap_or_default().to_uppercase();
    let path = request_line.next().unwrap_or_default().to_string();

    let mut headers = Vec::new();
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            headers.push((name.trim().to_lowercase(), value.trim().to_string()));
        }
    }

    let length = header_value(&headers, "content-length")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0)
        .min(MAX_BODY);

    let mut body = buffer[header_end..].to_vec();
    while body.len() < length {
        let read = stream.read(&mut chunk)?;
        if read == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..read]);
    }
    body.truncate(length);

    Ok(Some(Request { method, path, headers, body }))
}

fn forward(request: &Request, url: &str) -> Result<(u16, Vec<(String, String)>, Vec<u8>), String> {
    let mut outgoing = ureq::request(&request.method, url);
    for (name, value) in &request.headers {
        // The proxy speaks for the client: framing headers are rebuilt.
        if matches!(name.as_str(), "host" | "content-length" | "connection" | "accept-encoding" | "proxy-connection") {
            continue;
        }
        outgoing = outgoing.set(name, value);
    }

    let result = if request.body.is_empty() {
        outgoing.call()
    } else {
        outgoing.send_bytes(&request.body)
    };

    let response = match result {
        Ok(response) => response,
        Err(ureq::Error::Status(_, response)) => response,
        Err(error) => return Err(error.to_string()),
    };

    let status = response.status();
    let mut headers = Vec::new();
    for name in response.headers_names() {
        if let Some(value) = response.header(&name) {
            headers.push((name.clone(), value.to_string()));
        }
    }
    let mut body = Vec::new();
    response.into_reader().take(MAX_BODY as u64).read_to_end(&mut body).map_err(|error| error.to_string())?;
    Ok((status, headers, body))
}

fn write_response(stream: &mut TcpStream, status: u16, headers: &[(String, String)], body: &[u8]) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        _ => "OK",
    };
    let mut head = format!("HTTP/1.1 {status} {reason}\r\n");
    for (name, value) in headers {
        if matches!(name.as_str(), "content-length" | "connection" | "transfer-encoding" | "content-encoding") {
            continue;
        }
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str(&format!("content-length: {}\r\nconnection: close\r\n\r\n", body.len()));
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()
}

fn respond_json(stream: &mut TcpStream, status: u16, value: &Value) -> std::io::Result<()> {
    let body = value.to_string();
    let head = format!(
        "HTTP/1.1 {status} OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(body.as_bytes())?;
    stream.flush()
}

struct Counts {
    input: u64,
    output: u64,
    estimated: bool,
}

/// Reads the provider's own numbers when it sends them, and falls back to
/// counting words when it does not: roughly 4 bytes per token, one token per
/// 0.75 word. The fallback is labelled `estimated` so a dashboard can say so.
fn count_tokens(request_body: &[u8], response_body: &[u8], content_type: Option<&str>) -> Counts {
    let mut input = 0u64;
    let mut output = 0u64;
    let mut reported = false;

    let response_text = String::from_utf8_lossy(response_body);
    let streaming = content_type.map(|value| value.contains("event-stream")).unwrap_or(false);

    if streaming {
        for line in response_text.lines() {
            let line = line.trim_start_matches("data:").trim();
            if line.is_empty() || line == "[DONE]" {
                continue;
            }
            if let Ok(value) = serde_json::from_str::<Value>(line) {
                let (inp, outp) = usage_of(&value);
                input = input.max(inp);
                output = output.max(outp);
                reported |= inp > 0 || outp > 0;
            }
        }
    } else if let Ok(value) = serde_json::from_str::<Value>(&response_text) {
        let (inp, outp) = usage_of(&value);
        input = inp;
        output = outp;
        reported = inp > 0 || outp > 0;
    }

    if reported {
        return Counts { input, output, estimated: false };
    }

    let prompt = String::from_utf8_lossy(request_body);
    let words = word_count(&prompt);
    Counts {
        input: tokens_from_words(words),
        output: tokens_from_words(word_count(&response_text)),
        estimated: true,
    }
}

fn usage_of(value: &Value) -> (u64, u64) {
    let Some(usage) = value.get("usage") else { return (0, 0) };
    let take = |keys: &[&str]| -> u64 {
        keys.iter()
            .filter_map(|key| usage.get(*key).and_then(Value::as_u64))
            .max()
            .unwrap_or(0)
    };
    let input = take(&["prompt_tokens", "input_tokens"]) + take(&["cache_read_input_tokens"]);
    let output = take(&["completion_tokens", "output_tokens"]);
    (input, output)
}

fn word_count(text: &str) -> u64 {
    text.split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .count() as u64
}

fn tokens_from_words(words: u64) -> u64 {
    // ~0.75 tokens per word, the usual English ratio.
    (words * 3 + 2) / 4
}

fn record(path: &PathBuf, provider: &str, counts: &Counts) {
    let line = json!({
        "ts": SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0),
        "day": today(),
        "provider": provider,
        "input": counts.input,
        "output": counts.output,
        "estimated": counts.estimated
    });
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{line}");
    }
}

fn aggregate(path: &PathBuf) -> Value {
    let Ok(text) = fs::read_to_string(path) else { return json!({ "days": [], "totals": {} }) };
    let mut days: BTreeMap<String, BTreeMap<String, (u64, u64)>> = BTreeMap::new();
    let mut totals: BTreeMap<String, (u64, u64)> = BTreeMap::new();

    for line in text.lines() {
        let Ok(value) = serde_json::from_str::<Value>(line) else { continue };
        let day = value.get("day").and_then(Value::as_str).unwrap_or("").to_string();
        let provider = value.get("provider").and_then(Value::as_str).unwrap_or("unknown").to_string();
        let input = value.get("input").and_then(Value::as_u64).unwrap_or(0);
        let output = value.get("output").and_then(Value::as_u64).unwrap_or(0);
        let entry = days.entry(day).or_default().entry(provider.clone()).or_insert((0, 0));
        entry.0 += input;
        entry.1 += output;
        let total = totals.entry(provider).or_insert((0, 0));
        total.0 += input;
        total.1 += output;
    }

    let days: Vec<Value> = days
        .into_iter()
        .map(|(day, providers)| {
            let mut map = Map::new();
            for (provider, (input, output)) in providers {
                map.insert(provider, json!({ "input": input, "output": output }));
            }
            json!({ "day": day, "providers": map })
        })
        .collect();

    let totals: Map<String, Value> = totals
        .into_iter()
        .map(|(provider, (input, output))| (provider, json!({ "input": input, "output": output })))
        .collect();

    json!({ "days": days, "totals": totals })
}

fn today() -> String {
    let seconds = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let days = (seconds / 86_400) as i64;
    // Howard Hinnant's civil-from-days: no date crate for one line of arithmetic.
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    format!("{year:04}-{m:02}-{d:02}")
}

fn header_value(headers: &[(String, String)], name: &str) -> Option<String> {
    headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.clone())
}

fn strip_query(path: &str) -> &str {
    match path.find('?') {
        Some(position) => &path[..position],
        None => path,
    }
}

fn query_param(path: &str, name: &str) -> Option<String> {
    let query = path.split_once('?')?.1;
    for pair in query.split('&') {
        if let Some((key, value)) = pair.split_once('=') {
            if key == name && !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|window| window == needle)
}
