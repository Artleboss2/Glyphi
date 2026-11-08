/**
 * Turns the id a provider writes into its logs into the name people recognise:
 * `claude-sonnet-4-5-20250929` reads as "Claude Sonnet 4.5", `gpt-5-codex` as
 * "GPT-5 Codex". The raw id stays the grouping key, so two snapshots of the same
 * model never merge just because they print the same.
 */
const ACRONYMS = new Set(["gpt", "glm", "ai", "api", "llm", "moe", "vl", "sdk", "x"]);
const SMALL_WORDS = new Set(["of", "and", "the", "for"]);

export function prettyModelName(raw: string): string {
  const trimmed = raw.trim();
  if (!trimmed) return "Unknown model";
  // Vendor prefixes ("anthropic/claude…", "google.gemini…") name the company, not
  // the model, and the provider card already says which company it is.
  const tail = trimmed.split(/[/:]/).at(-1) ?? trimmed;
  // "google.gemini…" is a vendor prefix, but the dot in "glm-4.6" is part of the
  // version, so only a prefix without digits is dropped.
  const bare = /^[a-z][a-z-]*\./u.test(tail) ? tail.replace(/^[a-z][a-z-]*\./u, "") : tail;
  const undated = bare
    .replace(/-\d{8}$/u, "")
    .replace(/-\d{4}-\d{2}-\d{2}$/u, "")
    .replace(/@\d{8}$/u, "");
  const tokens = undated.split(/[-_\s]+/u).filter(Boolean);
  if (!tokens.length) return trimmed;

  let out = "";
  let previousNumeric = false;
  let previousWord = "";
  for (const token of tokens) {
    const lower = token.toLowerCase();
    const numeric = /^\d+(\.\d+)?$/u.test(token);
    const word = ACRONYMS.has(lower)
      ? lower.toUpperCase()
      // OpenAI's reasoning family (`o3`, `o4-mini`) is written lowercase.
      : /^o\d/u.test(lower) && !numeric
        ? lower
        : SMALL_WORDS.has(lower)
          ? lower
          : lower.charAt(0).toUpperCase() + lower.slice(1);
    if (!out) out = word;
    else if (numeric && previousNumeric) out += `.${word}`;
    // A brand acronym keeps its hyphen: "GPT-5", "GLM-4.6".
    else if (numeric && /^[A-Z0-9]+$/u.test(previousWord)) out += `-${word}`;
    else out += ` ${word}`;
    previousNumeric = numeric;
    previousWord = word;
  }
  return out;
}
