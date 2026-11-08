# Privacy policy

Glyphi is a local-first desktop application. It measures how much AI coding you do by reading files
that the tools you already use leave on your computer. It does not read your prompts, your source
code, your terminal history or your clipboard.

Last updated: 8 November 2026.

## What is stored on your computer

Everything the app needs to draw its screens lives in your user data folder:

- **Windows:** `%APPDATA%\Glyphi`
- **macOS:** `~/Library/Application Support/Glyphi`
- **Linux:** `~/.config/Glyphi`

That folder holds:

- `history.sqlite` — the local database: one record per provider and local day, containing token
  counts (input, cached input, cache creation, output, total), request counts, estimated cost,
  hourly buckets, per-model totals, the provider's quota snapshots and the local time zone;
- `desktop-preferences.json` — the settings you chose, including appearance, languages and which
  providers are enabled;
- `provider-credentials/` — provider credentials, encrypted with the operating system's protected
  storage (DPAPI on Windows, Keychain on macOS, libsecret on Linux). They are never uploaded;
- `app.ico`, `discovery/` and export files you asked the app to write.

Scans stay on the machine. Provider discovery and log parsing make **no network request**.

## What is sent when you enable cloud save

Cloud save is optional and off until you press **Save to cloud**. When it runs, it sends only the usage
records for the days you have collected:

- a hashed source identifier, the provider id, and the day in your local calendar;
- token and request totals, hourly buckets, model keys and the estimated cost;
- the completeness state of the day (`complete` or `open`) and its time zone.

It does **not** send prompts, completions, source code, file contents, file paths, project names,
credentials, email addresses or any other free text. The payload is validated before it leaves the
app, and a day is only marked complete once it is closed in your local calendar.

Restoring reads back the same records. You can delete a saved profile from the account page the app
links to.

## Anonymous product analytics

An **anonymous analytics** switch is on by default and can be turned off in **Settings → General**.
When it is on, the app sends a small number of product events (for example "app opened", "a provider
was enabled", "an update was checked") to a PostHog endpoint, together with:

- a random installation identifier stored in your preferences;
- the app version, operating system and CPU architecture;
- whether the build is packaged.

It never sends token counts, costs, provider names, model names, project names, handle names, file
paths or error contents. Geo-IP lookup is disabled. Turning the switch off stops the client
immediately.

## Update checks

The app asks GitHub for the latest release of the update channel you selected. The request carries no
account identity, only the user agent `Glyphi-Desktop`. A manual speed test of the network is never
performed.

## Your control

- Delete the user data folder to remove every local record. The app rebuilds it on the next launch.
- **Settings → Data** can export, redact project names and purge history older than a chosen number
  of days.
- Turning a provider off stops reading its files; its existing records stay until you purge them.
- Cloud save can be left off permanently; the desktop app is fully usable without an account.

## Changes to this policy

A material change will be noted in [CHANGELOG.md](CHANGELOG.md) and shown in the app before the new
behaviour is enabled. The current version of this document is the one in this repository.

## Contact

Privacy questions: open a discussion in <https://github.com/Artleboss2/Glyphi>. Security reports:
see [SECURITY.md](SECURITY.md).
