#!/usr/bin/env node
/**
 * Writes docs/PROVIDERS.md from the provider catalogue. The catalogue is the
 * source of truth for Settings → Providers, so the published list is generated
 * rather than maintained by hand.
 */
import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const source = await readFile(path.join(root, "apps/desktop/src/engine/discovery/catalog.ts"), "utf8");

const entries = [];
for (const match of source.matchAll(/^\s*\{\s*name:\s*"([^"]+)"(.*?)\},?\s*$/gmu)) {
  const [, name, rest] = match;
  const aliasBlock = rest.match(/aliases:\s*\[([^\]]*)\]/u);
  entries.push({
    name,
    aliases: aliasBlock ? [...aliasBlock[1].matchAll(/"([^"]+)"/gu)].map((alias) => alias[1]) : [],
    kind: rest.match(/kind:\s*"([a-z-]+)"/u)?.[1] ?? "other",
    adapter: rest.match(/adapter:\s*"([a-z-]+)"/u)?.[1] ?? null
  });
}

const titles = {
  "cli-agent": "Terminal agents and CLIs",
  editor: "Editors and IDE extensions",
  api: "API and model providers",
  local: "Local runtimes",
  builder: "App builders",
  observability: "Observability and gateways",
  assistant: "Assistant platforms"
};
const order = ["cli-agent", "editor", "api", "local", "builder", "observability", "assistant"];
const anchor = (title) => title.toLowerCase().replace(/[^a-z0-9 ]/gu, "").replace(/ /gu, "-");
const tracked = entries.filter((entry) => entry.adapter).length;
const names = entries.reduce((sum, entry) => sum + 1 + entry.aliases.length, 0);

const lines = [
  "# Supported providers",
  "",
  `Glyphi ships a directory of **${entries.length} AI developer tools**. ${tracked} have a local reader in the engine today; the rest are still detected on your computer, so the app can tell you whether it finds their store before you turn anything on.`,
  "",
  "A provider is *tracked* when the engine can read its usage from local files. Nothing is enabled without your consent: detection is passive, and the switch in Settings → Providers is the only thing that starts counting.",
  "",
  "| | |",
  "| --- | --- |",
  `| Products | ${entries.length} |`,
  `| Tracked today | ${tracked} |`,
  `| Names and aliases | ${names} |`,
  "",
  "## Contents",
  ""
];

for (const kind of order) {
  const group = entries.filter((entry) => entry.kind === kind);
  if (group.length) lines.push(`- [${titles[kind]}](#${anchor(titles[kind])}) — ${group.length}`);
}
lines.push("");

for (const kind of order) {
  const group = entries.filter((entry) => entry.kind === kind);
  if (!group.length) continue;
  lines.push(`## ${titles[kind]}`, "", "| Tool | Also known as | Reader |", "| --- | --- | --- |");
  for (const entry of [...group].sort((left, right) => left.name.localeCompare(right.name))) {
    lines.push(
      `| **${entry.name}** | ${entry.aliases.join(", ") || "—"} | ${entry.adapter ? `\`${entry.adapter}\`` : "detected only"} |`
    );
  }
  lines.push("");
}

lines.push(
  "---",
  "",
  "Generated from `apps/desktop/src/engine/discovery/catalog.ts` by `tooling/generate-providers-doc.mjs`. Add a tool there and rerun the script instead of editing this table."
);

await writeFile(path.join(root, "docs/PROVIDERS.md"), `${lines.join("\n")}\n`);
console.log(`${entries.length} providers (${tracked} tracked, ${names} names) written to docs/PROVIDERS.md`);
