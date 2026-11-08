# Contributing to Glyphi

Thanks for taking the time to improve Glyphi. This project is small and local-first; a focused pull
request that ships with a test is worth more than a large one that does not.

## Before you start

- Search the [issues](https://github.com/Artleboss2/Glyphi/issues) for the same problem. If there is
  none, open one describing what you see, what you expected, and which versions and operating system
  you used.
- For a new provider reader, include a **redacted** sample of the file you parse. Never attach a real
  transcript, an API key or anything with prompts or source code in it.
- For a security problem, follow [SECURITY.md](SECURITY.md) instead of opening an issue.

## Set up

Requirements: **Bun 1.3.14** and **Node.js 22.13+**.

```bash
git clone https://github.com/Artleboss2/Glyphi.git
cd Glyphi
bun install
bun run desktop:start
```

The workspace is a Bun + Turborepo monorepo:

- `apps/desktop` — the Electron app (main process, preload, engine, renderer);
- `packages/contracts` — the shared schemas, validators and fixtures;
- `tooling` — icon pipelines and repository scripts;
- `docs` — storage, discovery and provider references.

## Checks

Run the same checks CI runs before you push:

```bash
bun run check
```

That runs the tooling tests, ESLint, TypeScript in every package, the Vitest suites and the renderer
build. A change that only touches one package can use the scoped scripts, for example
`bun run desktop:typecheck` or `bun run desktop:test`.

New behaviour needs a test at the seam where it can fail: a parser gets a fixture, a store gets a
round-trip, a screen gets a render assertion. Do not weaken or delete an existing test to make a build
pass; explain the behaviour change in the pull request instead.

## Style

- Match the surrounding code: comments explain *why*, not *what the line does*.
- Prefer the simplest implementation that meets the requirement. No compatibility shims for old
  versions, no new dependency if the standard library does it.
- Keep the renderer free of Node APIs and the main process free of React.
- User-facing text is translated in `apps/desktop/src/renderer/i18n`. Add a key to every catalog and
  keep the five catalogs in sync; the test suite checks that they agree.
- Filenames, tabs and imports follow the existing conventions in the folder you edit.

## Commit and pull request

- Write commit messages that say why the change exists. One logical change per commit.
- Keep the subject under 72 characters; use the body for the reasoning and the trade-offs.
- In the pull request, describe the problem, the approach, and how you verified it. Attach a
  before/after screenshot for an interface change.
- Do not include generated build output, credentials, `.env` files or personal usage data.

## Licensing of contributions

By contributing, you agree that your contribution is licensed under the repository's
[PolyForm Noncommercial License 1.0.0](LICENSE), and that you have the right to submit it. There is no
separate contributor license agreement.
