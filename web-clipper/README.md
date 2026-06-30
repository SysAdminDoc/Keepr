# Keepr Web Clipper

Browser extension that saves the current page, article body, selection, link, or URL to your local Keepr database. **No internet roundtrip**: the extension talks only to `http://127.0.0.1:<port>/...` where Keepr is running locally.

## How it works

1. Keepr (the desktop app) spins up an HTTP server on a random localhost port at launch.
2. A 256-bit bearer token is generated on first launch and persisted in your Keepr database.
3. You paste the port + token into this extension's **Options** page.
4. Use the toolbar popup or right-click menu to choose what to save.
5. The extension extracts readable Markdown locally, POSTs to Keepr, and Keepr inserts a new note.

The token is **manual paste only** by design. Auto-discovery would let any local process pair with Keepr. See `src-tauri/src/web_clipper.rs` in the Keepr repo for the threat model.

## Install from a Keepr release

### Chrome / Edge

1. Download `Keepr-Web-Clipper-<version>.zip` from the Keepr GitHub Release.
2. Extract it to a permanent folder; the browser loads the extension from that folder on every startup.
3. Open `chrome://extensions` (or `edge://extensions`).
4. Enable **Developer mode**.
5. Click **Load unpacked** and select the extracted folder.
6. Click the puzzle icon -> pin **Keepr Web Clipper**.

The release may also include `Keepr-Web-Clipper-<version>.crx`, but modern Chrome/Edge reject self-hosted CRX drag-and-drop installs. Treat the CRX as a secondary artifact for enterprise/manual tooling; normal installs should use the ZIP and **Load unpacked**.

### Firefox

1. Open `about:debugging#/runtime/this-firefox`.
2. Click **Load Temporary Add-on...**
3. Select `web-clipper/manifest.json`.

Note: Temporary add-ons disappear on Firefox restart. For permanent install, package as `.xpi` and sign via AMO (not yet automated).

## Build locally

From the repo root:

```sh
npm run build:clipper
```

The script writes `dist-web-clipper/Keepr-Web-Clipper-<version>.zip` with POSIX archive paths for **Load unpacked**, then creates `dist-web-clipper/Keepr-Web-Clipper-<version>.crx` using a local gitignored `keepr-web-clipper-selfhost.pem` key. It re-parses the ZIP and CRX before exiting to verify required files, forward-slash paths, CRX3 magic/version, ZIP payload, and RSA-SHA256 signature.

## Pair with Keepr

1. Open Keepr -> Settings -> **Web Clipper**.
2. Copy the **Port** value.
3. Copy the **Bearer token**.
4. Click the extension's Options link -> paste both -> **Save** -> **Test connection**.

If Test Connection succeeds, a "Keepr Web Clipper test" note appears in your Keepr. Delete it; from now on the toolbar and context-menu actions work.

## What gets saved

- **Save article**: page title, source URL, meta description, and a cleaned Markdown rendering of the best article/main/content node.
- **Save selection**: the selected text or selected HTML converted to Markdown.
- **Save URL only**: just the active tab URL + title.
- **Right-click -> Save page to Keepr**: same article extraction as the toolbar.
- **Right-click -> Save selection to Keepr**: selected content with `clipped` and `selection` labels.
- **Right-click -> Save link to Keepr**: the link target with `clipped` and `link` labels.

Every clip becomes a text note with `Source: <url>` as the first line. Article, selection, and link clips also receive mode-specific labels.

## Privacy

- The extension only ever fetches `http://127.0.0.1:<your-port>/...`.
- No telemetry, no analytics, no CDN-loaded scripts.
- The bundled `manifest.json` declares `host_permissions: ["http://127.0.0.1/*"]` only.
- `activeTab` + `scripting` means page access is gated on a toolbar click or context-menu action. Keepr still avoids an `<all_urls>` install warning.

## Current limits

- **Screenshot clip**: needs `tabCapture` permission, which scares browsers; defer until users ask.
- **Tag picker**: clips receive automatic labels; tag editing is via the Keepr UI.

See the top-level repo `ROADMAP.md` and `CHANGELOG.md` for the broader plan.
