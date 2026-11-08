# Security policy

## Reporting a vulnerability

Please do **not** open a public issue for a security problem.

Report it privately through GitHub's advisory flow:
**Security → Advisories → Report a vulnerability** on
<https://github.com/Artleboss2/Glyphi/security/advisories/new>.

Include what you were doing, the version shown in **Settings → About**, the operating system, and the
smallest reproduction you can build. A proof of concept is welcome; please do not test against other
people's machines or accounts.

You can expect:

- an acknowledgement within **72 hours**;
- an assessment and a remediation plan within **7 days**;
- credit in the advisory and the release notes, unless you prefer to stay anonymous.

## Supported versions

Security fixes ship in the newest release and, when the fix is small enough to backport, in the
previous minor line. Builds older than the two most recent releases are not supported: they may read
local files with outdated parsers and cannot receive an update.

## Scope

Glyphi runs on the user's computer and reads local files. The following are in scope:

- reading files outside the documented provider locations;
- path traversal, symlink or archive handling in imports, exports and updates;
- command or argument injection in the engine, updater or shell integrations;
- bypassing the sandbox, context isolation or the renderer CSP;
- leaking prompts, source code, credentials or project paths;
- authentication or authorization flaws in the optional cloud save.

Out of scope: a provider changing its own log format, rate limits or prices; the accuracy of a
provider's published numbers; anything that requires an already-compromised operating system account.

## What the app never needs

Glyphi never asks for a password, an SSH key, a signing certificate or a GitHub token. Credentials it
does use for provider quotas stay in the operating system's encrypted store and are excluded from
cloud saves. If a message asks for one of these in Glyphi's name, treat it as phishing.
