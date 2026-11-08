# Changelog

All notable changes to Glyphi are recorded here. Releases follow semantic versioning for the desktop
app; the local database schema and the cloud document format carry their own versions.

## Unreleased — 0.4.7

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

- Light, dark and system themes, four type families, a type scale, ink and accent colours, glass or
  solid surfaces and configurable motion.
- Three app marks, three mini-window densities, left-corner placement and a mini window that resizes
  without clipping.

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
