# Keepr Roadmap

Actionable work only. Historical and completed roadmap material is archived in CHANGELOG.md; blocked work is kept in Roadmap_Blocked.md.

## Actionable Items

- [ ] **P2 — Sync: plaintext HTTP** — all sync traffic is unencrypted. Consider TLS or at least transport-layer encryption for note content.
  Where: `src-tauri/src/commands/sync.rs`, `src-tauri/src/sync/server.rs`

- [ ] **P2 — App Lock: no brute-force throttling** — 4-digit PIN exhaustible in ~1 hour at 3-6 attempts/sec with no lockout.
  Where: `src-tauri/src/commands/security.rs:109-116`

- [ ] **P2 — Takeout import: no aggregate size limit** — unlike `import_zip` which caps at 2 GiB uncompressed, `import_takeout` loads all entries into memory with no cap.
  Where: `src-tauri/src/commands/io.rs:1070-1099`

- [ ] **P2 — Scanner: no image size cap** — loading a 100+ megapixel photo can OOM the tab. Should cap source to ~4096px longest edge.
  Where: `src/components/DocumentScannerModal.tsx:398-418`

- [ ] **P2 — Scanner: `Array.from(Uint8Array)` memory amplification** — 16-24x memory expansion when converting PNG to number[]. Consider base64 or chunked transfer.
  Where: `src/components/DocumentScannerModal.tsx:660`, `src/components/NoteEditor.tsx`

- [ ] **P2 — Scanner: off-screen canvases accumulate** — `applyFilter` + `drawPreview` create canvases on every filter change without cleanup.
  Where: `src/components/DocumentScannerModal.tsx`

- [ ] **P2 — Scanner: OpenCV `cv.Size` / `contours.get()` WASM heap leaks** — Emscripten heap objects not freed in `detectEdges` and `warpDoc`.
  Where: `src/components/DocumentScannerModal.tsx:92,106,109,167`

- [ ] **P2 — FilterChips: no click-outside or Escape-key dismissal** — dropdown menus trap keyboard users.
  Where: `src/components/FilterChips.tsx:184-361`

- [ ] **P2 — mDNS daemons never shut down** — `ServiceDaemon` handles dropped without `unregister`; browser thread has no shutdown signal.
  Where: `src-tauri/src/sync/discovery.rs`, `src-tauri/src/lib.rs`

- [ ] **P3 — `color` field has no server-side validation** — accepts arbitrary strings unlike `background_pattern` which is whitelisted.
  Where: `src-tauri/src/commands/notes.rs:1002-1012`

- [ ] **P3 — Sidebar: smart label delete uses `window.confirm` instead of `ConfirmDialog`** — inconsistent with rest of app.
  Where: `src/components/Sidebar.tsx:73`

- [ ] **P3 — ICS export: `escape_ics` does not fold long lines per RFC 5545** — some calendar apps may reject/truncate.
  Where: `src-tauri/src/commands/reminders.rs:237-243`

---

- [ ] **Code-signing (v0.5+):** ship unsigned with SmartScreen workaround until Azure Trusted Signing subscription approved.

- [ ] **macOS / Linux support tier (v0.10+):** Windows is supported; macOS + Linux are best-effort.
