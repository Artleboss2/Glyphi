# Changelog

All notable changes to Glyphi are recorded here. Releases follow semantic versioning for the desktop
app; the local database schema and the cloud document format carry their own versions.

## 0.5.2 — 2026-11-12

### Fixed

- **Cloud save stops calling a large batch corrupt.** The API refused any request body over 512 KiB
  and answered `invalid_records`, which read like damaged data. An oversized body now answers
  `payload_too_large`, the limit is 2 MiB, a refused record names its index and field, and both
  record envelopes are accepted.
- **A save batch is measured by the body it becomes.** Records were sized one at a time, without the
  JSON envelope or the commas between them, so a full batch could cross the server's limit without
  anything noticing. Batches are budgeted at 384 KiB of the real body, and a restore response has
  its own limit instead of borrowing the save one.
- **The card picker's dropdown is readable.** Native options inherited cream text on a dark page and
  looked ghosted out.

### Added

- **Your own card colours.** `accent` and `cell` on a card URL, with pickers in the website builder
  and in the app. The heatmap ramp is generated from one colour, and invalid values are ignored.
- **Features, pricing and FAQ pages**, an `llms.txt`, one schema.org graph and a wider sitemap.

### Changed

- **The beta badge is gone.** Glyphi has saved, restored and published enough to drop it from the
  header and the footer.
- **Both card builders open in dark**, matching the app and the site.

## 0.5.1 — 2026-11-10

### Fixed

- **Your public profile finally shows your usage.** Cloud save held back every source that was not
  on a fixed shortlist, and Freebuff — where nearly all of a typical history lives — was not on it.
  Saves reported success while the day records stayed on your computer, so glyphi.xyz/@you stayed
  empty. Every source this build collects is now uploaded under its own name, and the misleading
  "stays on this computer" notice is gone.
- **Cloud save stops rejecting the whole batch.** A day record that had never been uploaded was sent
  with revision 0, which the API refuses, so one new day made every save fail with
  `invalid records`. New records now claim revision 1, and a refusal reports the server's own
  reason instead of a bare status code.
- **An empty account receives the whole history.** A save on an account the server had never seen
  only ever queued the days collected since sign-in. Each save now reads the account first; when it
  holds nothing, the entire local history is queued and uploaded.

### Changed

- **Three release pipelines instead of one.** Windows, macOS and Linux now build and publish
  independently, so a platform that cannot build no longer fails the others. Windows assets, macOS
  assets and Linux assets each arrive on the same release with their own `SHA256SUMS-<platform>`.
- **macOS ships again, unsigned.** The Apple secrets are optional now: without them the build is
  compiled, published, and marked unsigned instead of being skipped. With them it signs and
  notarizes exactly as before.
- **Linux is in the release notes, not an afterthought.** The README states the AppImage, `.deb` and
  `.rpm` targets that have shipped since 0.4.

## 0.5.0 — 2026-11-10

### Cloud

- **Sign in with GitHub, straight from the app.** Settings → Cloud now offers *Sign in with GitHub*
  beside the email form. The app listens on a loopback port, opens your browser, and GitHub comes
  back to that port with a one-time ticket; the app trades the ticket for its own session. Your
  password never passes through the app, and no token ever crosses the browser.

### Fixed

- **A release no longer fails on a platform we cannot sign.** macOS builds need an Apple certificate
  to be distributable, and without one the whole run failed and took the release with it. The run
  now skips macOS when those secrets are absent, and publishes what it did build.

## 0.4.9 — 2026-11-09

### Removed

- **The Attribution page is gone.** The rail no longer carries it and reporting mixes are no longer
  editable in the app. Cloud save still uploads everything the server models; a source it does not
  model stays on this computer, and the copy that used to point at the page now says so.

### Fixed

- **Share cards no longer overflow.** The recap image is a fixed 1200 × 675 canvas, but its total
  followed the Appearance setting, so "150,503,609" at 124 px ran over the chart beside it. The card
  now always uses the short form ("150.5M"), whatever the interface is set to.

- **Sign-in works.** The approval page used to open a `*.supabase.co` URL that rendered as raw HTML:
  Supabase forces `text/plain` and a sandbox CSP on HTML from an Edge Function. The email and
  password form now lives in **Settings → Cloud**, the function mints the session directly, and the
  device-code flow (page, routes and table) is gone.
- **Appearance settings stick.** `theme` was missing from the patch allowlist, so every change on the
  Appearance tab was rejected after it had already been painted: the theme flicked back and the
  selected tile never moved. The validator now lives in `shared/appearance-patch.ts` with a test that
  covers every field.
- **The app no longer quits in the background.** Releasing the hidden windows for memory counted as
  "the last window closed", so a minimized or tray-bound app quit a minute later. That release no
  longer triggers a quit; the tray, the shortcut and the hotkey all bring the window back.

## 0.4.8 — 2026-11-09

### Cloud

- **Cloud save runs on Glyphi's own database.** The account service is now a Supabase project
  (`eu-west-3`) holding a hardened Postgres schema: every table sits in a private schema with RLS on
  and no policies, `anon` and `authenticated` have no privileges, and each read or write goes through
  a `SECURITY DEFINER` function that only the service role may execute. Session tokens are stored as
  SHA-256 hashes, device codes are single-use, and every endpoint is rate limited.
- **A new settings tab.** **Settings → Cloud** shows the connected account, the hourly auto-save
  switch, save and restore, sync progress, held-back sources and any conflict to resolve. The
  profile page can open it, so the "turn it on in Settings" hint now leads somewhere.
- **Sign-in is a browser page, not a website.** The app shows a code, opens an approval page served
  by the API itself, and receives a 90-day session once you confirm. A new email creates the account.
- **No UsageAtlas URLs remain.** The cloud origin, the profile links, the embed card URLs, the share
  links, the CSP image source and the schema id all point at Glyphi (`glyphi.xyz`) or the API.

## 0.4.7 — 2026-11-08

### Rebranding

- **The product is now Glyphi.** The window, tray, notifications, installer, updater, documentation
  and source tree carry one name, one repository and one licence. Every reference to the previous
  identity is gone from the code and the assets.
- **A new mark.** The application icon is the glass Eye of Horus tile: the surrounding frame and the
  white border are removed, the tile keeps its transparent rounded corners, and every size — PNG, ICO
  (16–256 px), ICNS, favicon, installer and the three selectable marks — is rendered from the single
  `branding/glyphi-primary.png` master.
- **An Egyptian interface.** The icon set is redrawn around sun discs, stelae, obelisks, cartouches,
  ankhs, scarabs and the scales of Maat, and the typography moves to Cinzel, Marcellus and Cormorant
  Garamond, bundled as local fonts.
- **A new baseline scene.** The eleven photographic and vector wallpapers are replaced by one desert
  dune background, shipped with the app.

### Fixed

- **Cloud save no longer stops on a false failure.** A record the server already held at the same
  revision was skipped silently; the next pass found it unchanged and aborted the remaining uploads
  with `Cloud save made no progress`. It is now a resolvable conflict and the rest of the batch
  completes.
- **Attribution follows the token mass.** The distribution wheel splits tokens, not requests, so a few
  heavy requests can no longer pull a profile away from the saved rule. The screen shows both the
  rule and the distribution it produces.

### Changed

- **One providers screen.** The source list, the credential forms, the search and the scan now live
  together in **Settings → Providers**; the duplicate copy in General is gone, and the scan button
  sits next to the search it belongs to.
- **The exchange rate is no longer a setting.** Costs are converted with a dated rate table shipped
  with the app, so two people comparing screenshots no longer see different totals for the same
  tokens. Settings shows the rate and the date it is from.
- **Model names read like names.** `claude-sonnet-4-5-20250929` is shown as *Claude Sonnet 4.5* and
  `gpt-5.6-sol` as *GPT-5.6 Sol*. The raw id stays the grouping key, so two snapshots of one model
  never merge just because they print the same.
- **Five Egyptian wallpapers** instead of one: Dunes, Nile at dusk, Papyrus, Lapis & gold and Aten
  sunrise.
- Light, dark and system themes, four type families, a type scale, ink and accent colours, glass or
  solid surfaces and configurable motion.
- Three app marks, three mini-window densities, left-corner placement and a mini window that resizes
  without clipping.
- A launch beat: the eye blinks, glances right, glances left, turns red and sweeps the window with a
  laser before the app appears. Any click or Escape skips it, and it never runs with reduced motion.

### Performance

- **SQLite page caches cut from 64 MiB to 8 MiB per connection** (2 MiB for readers) and the WAL is
  capped at 16 MiB, so the engine, its writer thread and a background reader no longer reserve a few
  hundred megabytes on an idle machine. Batched writes, not a large page cache, are what keep the
  import fast, and `PRAGMA optimize` now runs on open.

### Documentation

- [docs/PROVIDERS.md](docs/PROVIDERS.md) lists every supported tool with what is read from it.
- The README shows the dashboard and links the provider list.

## 0.4.6 — 2026-10-30

- **Quiet in the tray.** Scans slow down when the window is hidden, animations park, and battery mode
  reduces polling further.
- **A provider directory.** The 300-tool catalog is searchable in Settings; enabling a tool is always
  an explicit action.
- **Discovery remembers the route** and keeps looking: remembered files are re-probed, their folders
  are walked again and a broken route is planned from scratch.
- Ten 0.5.0 objectives published with acceptance criteria.

## 0.4.5 — 2026-10-29

- The cloud save error that held back providers the server did not model is fixed.
- Day details use the readable record shape again instead of the compressed one.
- The mini window keeps its rounded card, resizes from real handles and no longer shows the page
  behind it.
- The taskbar icon survives an update: old `app-*` folders are cleaned without leaving a shortcut
  pointing at a deleted file.

## 0.4.4 — 2026-10-28

- A settings search that finds every option by name.
- A quieter mini window, reduced motion and lower idle CPU.
- Cost estimates separated from token estimates on every card.

## 0.4.3 — 2026-10-27

- Active tool capacity: a meter per tool with its quota window and reset countdown.
- Cost estimates recalibrated to a per-million-token basis, with cache reads and writes priced
  separately.
- Larger achievements for 0.5.0 and a visual timeline.

## 0.4.2 — 2026-10-26

- In-app updates that install in place on Windows.
- Installer artwork, update feed and a rollback path verified end to end.
- Release workflow publishes the installer, Squirrel package, portable zip and checksums.

## 0.4.1 — 2026-10-25

- Every setting in one screen, grouped by what it changes.
- Provider discovery gains a manual folder picker.
- Diagnostics report the engine version and the files it read.

## 0.4.0 — 2026-10-24

- Native local readers for Codex, Claude Code, Cursor, OpenCode and the transcript-based tools.
- A compact history schema and a one-time compaction pass.
- The desktop dashboard: today, history, trends, insights, providers and limits.
