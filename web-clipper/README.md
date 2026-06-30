# Keepr Web Clipper

Browser extension that saves the current page (or selection, or URL) to your local Keepr database. **No internet roundtrip** — the extension talks only to `http://127.0.0.1:<port>/...` where Keepr is running locally.

## How it works

1. Keepr (the desktop app) spins up an HTTP server on a random localhost port at launch.
2. A 256-bit bearer token is generated on first launch and persisted in your Keepr database.
3. You paste the port + token into this extension's **Options** page.
4. Click the toolbar button → choose what to save → the extension POSTs to Keepr → Keepr inserts a new note.

The token is **manual paste only** by design — auto-discovery would let any local process pair with Keepr. See `src-tauri/src/web_clipper.rs` in the Keepr repo for the threat model.

## Install from a Keepr release

### Chrome / Edge

1. Download `Keepr-Web-Clipper-<version>.zip` from the Keepr GitHub Release.
2. Extract it to a permanent folder; the browser loads the extension from that folder on every startup.
3. Open `chrome://extensions` (or `edge://extensions`).
4. Enable **Developer mode** (top-right toggle).
5. Click **Load unpacked** and select the extracted folder.
6. Click the puzzle icon -> pin **Keepr Web Clipper**.

The release may also include `Keepr-Web-Clipper-<version>.crx`, but modern Chrome/Edge reject self-hosted CRX drag-and-drop installs. Treat the CRX as a secondary artifact for enterprise/manual tooling; normal installs should use the ZIP and **Load unpacked**.

### Firefox

1. Open `about:debugging#/runtime/this-firefox`.
2. Click **Load Temporary Add-on…**
3. Select `web-clipper/manifest.json`.

Note: Temporary add-ons disappear on Firefox restart. For permanent install, package as `.xpi` and sign via AMO (not yet automated).

## Build locally

From the repo root:

```sh
npm run build:clipper
```

The script writes `dist-web-clipper/Keepr-Web-Clipper-<version>.zip` with POSIX archive paths for **Load unpacked**, then creates `dist-web-clipper/Keepr-Web-Clipper-<version>.crx` using a local gitignored `keepr-web-clipper-selfhost.pem` key. It re-parses the ZIP and CRX before exiting to verify required files, forward-slash paths, CRX3 magic/version, ZIP payload, and RSA-SHA256 signature.

## Pair with Keepr

1. Open Keepr → Settings → **Web Clipper**.
2. Copy the **Port** value (5-digit number).
3. Copy the **Bearer token** (64 hex characters).
4. Click the extension's Options link → paste both → **Save** → **Test connection**.

If Test Connection succeeds, a "Keepr Web Clipper test" note appears in your Keepr — delete it; from now on the toolbar button works.

## What gets saved

- **Save full page** — page title + URL + meta description + first 4 KB of visible body text (no Readability for v0.1; clip-by-snippet is plenty for highlights and reference saves).
- **Save selection** — only the text you currently have highlighted.
- **Save URL only** — just the URL + title. Fastest; no DOM access.

Every clip becomes a text note with `Source: <url>` as the first line and an auto-applied `clipped` label.

## Privacy

- The extension only ever fetches `http://127.0.0.1:<your-port>/...`.
- No telemetry, no analytics, no CDN-loaded scripts.
- The bundled `manifest.json` declares `host_permissions: ["http://127.0.0.1/*"]` only.
- `activeTab` + `scripting` permission means content-script access is gated on YOUR click — no `<all_urls>` install warning.

## Current limits

- **Readability.js-powered article extraction** - will land when the simpler innerText snippet stops being good enough.
- **Screenshot clip** — needs `tabCapture` permission, which scares browsers; defer until users ask.
- **Tag picker** — every clip currently lands with `clipped`; tag editing is via the Keepr UI.
- **Right-click context menu items** — toolbar-only for now.

See the top-level repo `ROADMAP.md` and `CHANGELOG.md` for the broader plan.
