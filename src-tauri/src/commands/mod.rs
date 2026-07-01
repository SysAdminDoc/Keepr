use crate::AppState;
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{Manager, State};
use uuid::Uuid;
use zip::write::SimpleFileOptions;

pub mod attachments;
pub mod history;
pub mod io;
pub mod labels;
pub mod notes;
pub mod reminders;
pub mod security;
pub mod sync;

#[cfg(test)]
use attachments::*;
#[cfg(test)]
use io::*;
#[cfg(test)]
use notes::*;
#[cfg(test)]
use reminders::*;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ChecklistItem {
    pub id: String,
    pub text: String,
    pub checked: bool,
    pub position: i64,
    /// NF-V0.5-21 (v0.14+): one-level nesting. When set, this item is
    /// indented under the referenced sibling. Validated server-side so
    /// the referenced parent itself has `parent_id = None` (Keep parity
    /// — only one level deep). Defaults absent for plain top-level items.
    #[serde(default)]
    pub parent_id: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    pub id: String,
    pub note_id: String,
    pub kind: String, // "image" | "drawing" | "audio" | "file"
    pub mime: String,
    pub filename: String,
    pub byte_size: i64,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub position: i64,
    pub created_at: String,
    #[serde(default)]
    pub resource_path: Option<String>,
    #[serde(default)]
    pub thumb_path: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Note {
    pub id: String,
    pub kind: String, // "text" | "list"
    pub title: String,
    pub body: String,
    pub color: String,
    pub pinned: bool,
    pub archived: bool,
    pub trashed: bool,
    pub position: i64,
    pub created_at: String,
    pub updated_at: String,
    pub trashed_at: Option<String>,
    pub checklist: Vec<ChecklistItem>,
    pub labels: Vec<String>, // label IDs
    pub attachments: Vec<Attachment>,
    /// Count of legacy/plaintext attachment rows withheld from a vault
    /// note. Vaulted attachments are not encrypted, so Rust never returns
    /// their resource paths to the renderer.
    #[serde(default)]
    pub vault_attachment_count: usize,
    /// NF-V0.5-C — "plain" or "vault". When "vault" + DEK is unlocked,
    /// title/body/checklist are decrypted before being returned. When
    /// "vault" + DEK is locked, the renderer shows a "🔒 Locked" card.
    #[serde(default = "default_vault_state")]
    pub vault: String,
    /// NF-22 (v0.14+): pattern key from the renderer-side whitelist
    /// (`src/lib/backgroundPatterns.ts`). Empty string = no pattern.
    /// Unknown values map to "none" client-side without error.
    #[serde(default)]
    pub background_pattern: String,
}

fn default_vault_state() -> String {
    "plain".to_string()
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Label {
    pub id: String,
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct NoteInput {
    pub kind: String,
    pub title: String,
    pub body: String,
    pub color: String,
    pub pinned: bool,
    pub checklist: Vec<ChecklistItemInput>,
    pub labels: Vec<String>,
    /// NF-22 (v0.14+): pattern key or "" for none. Validated to be one
    /// of the known whitelist values (or empty) so a renderer bug can't
    /// land an arbitrary string in the column.
    #[serde(default)]
    pub background_pattern: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ChecklistItemInput {
    pub id: Option<String>,
    pub text: String,
    pub checked: bool,
    pub position: i64,
    /// NF-V0.5-21 (v0.14+): when set, references another item in the
    /// same `checklist` input array by its `id`. Validated by
    /// `validate_note_input` — must be present in the same array, and
    /// that referenced item must itself have no parent (one level).
    #[serde(default)]
    pub parent_id: Option<String>,
}

fn now_iso() -> String {
    Utc::now().to_rfc3339()
}

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write as _};
    use zip::write::SimpleFileOptions;

    fn build_zip<F: FnOnce(&mut zip::ZipWriter<Cursor<Vec<u8>>>)>(build: F) -> Vec<u8> {
        let buf = Cursor::new(Vec::<u8>::new());
        let mut zw = zip::ZipWriter::new(buf);
        build(&mut zw);
        zw.finish().unwrap().into_inner()
    }

    #[test]
    fn app_version_comes_from_package_metadata() {
        assert_eq!(get_app_version(), env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn validate_accepts_a_normal_backup() {
        let bytes = build_zip(|zw| {
            let opts = SimpleFileOptions::default();
            zw.start_file("keepr.db", opts).unwrap();
            zw.write_all(b"SQLite format 3\0").unwrap();
            zw.start_file("resources/abc.png", opts).unwrap();
            zw.write_all(b"PNGDATA").unwrap();
        });
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        validate_zip_archive(&mut archive).expect("should accept normal backup");
    }

    #[test]
    fn validate_rejects_too_many_entries() {
        let bytes = build_zip(|zw| {
            let opts = SimpleFileOptions::default();
            for i in 0..(MAX_ENTRY_COUNT + 1) {
                zw.start_file(format!("entry-{i}"), opts).unwrap();
            }
        });
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        let err = validate_zip_archive(&mut archive).unwrap_err();
        assert!(err.contains("max"), "got: {err}");
    }

    #[test]
    fn validate_rejects_path_traversal() {
        // A zip with `..\..\evil.txt` cannot be created via start_file's
        // sanitization, but raw zip parsers will accept it. We construct
        // such a malicious zip by hand.
        let mut raw = Vec::<u8>::new();
        // Use the zip crate's raw API: start_file_from_path with a literal
        // name that includes `..`. zip-rs will pass it through; the validator
        // must catch it via enclosed_name().
        let mut zw = zip::ZipWriter::new(Cursor::new(&mut raw));
        let opts = SimpleFileOptions::default();
        // The crate sanitizes via mangle on read, so enclosed_name will be
        // None for `../escape.txt` because it resolves outside the root.
        zw.start_file("../escape.txt", opts).unwrap();
        zw.write_all(b"hi").unwrap();
        zw.finish().unwrap();

        let mut archive = zip::ZipArchive::new(Cursor::new(raw)).unwrap();
        let err = validate_zip_archive(&mut archive).unwrap_err();
        assert!(err.contains("unsafe path"), "got: {err}");
    }

    // --- Pure helpers ---

    #[test]
    fn sanitize_extension_handles_uppercase_and_specials() {
        use std::path::Path;
        assert_eq!(sanitize_extension(Path::new("foo.PNG")), "png");
        assert_eq!(sanitize_extension(Path::new("foo.JPEG")), "jpeg");
        // No extension → default
        assert_eq!(sanitize_extension(Path::new("README")), "bin");
        // Non-alphanumeric characters dropped
        assert_eq!(sanitize_extension(Path::new("foo.t!@#xt")), "txt");
        // Truncated at 8 chars
        assert_eq!(sanitize_extension(Path::new("foo.abcdefghij")), "abcdefgh");
    }

    #[test]
    fn sanitize_vault_filename_strips_unsafe_chars() {
        assert_eq!(
            sanitize_vault_filename("Hello / World: <test>", "abc12345"),
            "Hello - World- -test-",
        );
        // Pure-unsafe input that collapses to nothing falls back to
        // "note-<short id>". (Slashes/asterisks become dashes, not
        // empty — so we need actual nothings: empty input.)
        assert_eq!(
            sanitize_vault_filename("", "abc12345-rest-of-uuid"),
            "note-abc12345",
        );
        // Trims leading dots/spaces
        assert_eq!(sanitize_vault_filename("  .hidden  ", "xyz"), "hidden",);
    }

    #[test]
    fn yaml_quote_if_needed_quotes_special() {
        assert_eq!(yaml_quote_if_needed("safe"), "safe");
        assert_eq!(yaml_quote_if_needed("with: colon"), "\"with: colon\"");
        assert_eq!(
            yaml_quote_if_needed("- starts-with-dash"),
            "\"- starts-with-dash\""
        );
        assert_eq!(yaml_quote_if_needed(""), "\"\"");
        // Backslash + quote escape
        assert_eq!(
            yaml_quote_if_needed("she said \"hi\""),
            "\"she said \\\"hi\\\"\""
        );
    }

    const TEST_ONE_BY_ONE_PNG: &[u8] = &[
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f,
        0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xf8,
        0xcf, 0xc0, 0xf0, 0x1f, 0x00, 0x05, 0x00, 0x01, 0xff, 0xa7, 0x69, 0x6c, 0x6d, 0x00, 0x00,
        0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
    ];

    #[test]
    fn parse_markdown_vault_note_reads_frontmatter_checklist_and_resources() {
        let raw = concat!(
            "---\n",
            "id: 11111111-1111-4111-8111-111111111111\n",
            "type: list\n",
            "color: blue\n",
            "pinned: true\n",
            "archived: false\n",
            "created: 2026-01-02T03:04:05Z\n",
            "updated: 2026-01-03T03:04:05Z\n",
            "labels:\n",
            "  - Work\n",
            "  - \"two: words\"\n",
            "---\n",
            "\n",
            "# Grocery list\n",
            "\n",
            "- [ ] Milk\n",
            "- [x] Bread\n",
            "\n",
            "![pixel](_resources/pixel.png)\n"
        );

        let draft = parse_markdown_vault_note(raw, "ignored");

        assert_eq!(
            draft.frontmatter.id.as_deref(),
            Some("11111111-1111-4111-8111-111111111111")
        );
        assert_eq!(draft.frontmatter.labels, vec!["two: words", "Work"]);
        assert_eq!(draft.kind, "list");
        assert_eq!(draft.title, "Grocery list");
        assert_eq!(draft.checklist.len(), 2);
        assert_eq!(draft.checklist[0].text, "Milk");
        assert!(!draft.checklist[0].checked);
        assert_eq!(draft.checklist[1].text, "Bread");
        assert!(draft.checklist[1].checked);
        assert_eq!(draft.resource_refs.len(), 1);
        assert_eq!(draft.resource_refs[0].target, "_resources/pixel.png");
        assert_eq!(draft.resource_refs[0].filename, "pixel");
    }

    #[test]
    fn parse_markdown_vault_note_reads_obsidian_resource_embeds() {
        let draft = parse_markdown_vault_note(
            "# Obsidian\n\nBody\n\n![[attachments/pixel.png|Reference image]]\n",
            "Obsidian",
        );

        assert_eq!(draft.resource_refs.len(), 1);
        assert_eq!(draft.resource_refs[0].target, "attachments/pixel.png");
        assert_eq!(draft.resource_refs[0].filename, "Reference image");
    }

    #[test]
    fn import_markdown_vault_creates_notes_labels_lists_and_attachments() {
        let tmp = tempfile::tempdir().unwrap();
        let data_dir = tmp.path().join("data");
        let vault_dir = tmp.path().join("vault");
        let resources_dir = vault_dir.join(VAULT_RESOURCES_DIR);
        std::fs::create_dir_all(&data_dir).unwrap();
        std::fs::create_dir_all(&resources_dir).unwrap();
        std::fs::write(resources_dir.join("pixel.png"), TEST_ONE_BY_ONE_PNG).unwrap();
        std::fs::write(
            vault_dir.join("Grocery list.md"),
            concat!(
                "---\n",
                "id: 11111111-1111-4111-8111-111111111111\n",
                "type: list\n",
                "color: blue\n",
                "pinned: true\n",
                "archived: true\n",
                "created: 2026-01-02T03:04:05Z\n",
                "updated: 2026-01-03T03:04:05Z\n",
                "labels:\n",
                "  - Work\n",
                "  - \"two: words\"\n",
                "---\n",
                "\n",
                "# Grocery list\n",
                "\n",
                "- [ ] Milk\n",
                "- [x] Bread\n",
                "\n",
                "![pixel](_resources/pixel.png)\n"
            ),
        )
        .unwrap();
        let mut conn = crate::db::open(&data_dir.join("keepr.db")).unwrap();

        let summary = import_markdown_vault_inner(&mut conn, &data_dir, &vault_dir).unwrap();

        assert_eq!(summary.notes_created, 1);
        assert_eq!(summary.attachments_copied, 1);
        assert_eq!(summary.labels_created, 2);
        assert!(summary.skipped_files.is_empty(), "{summary:?}");
        assert!(summary.collisions.is_empty(), "{summary:?}");
        let note: (String, String, String, i64, i64, String, String) = conn
            .query_row(
                "SELECT kind, title, color, pinned, archived, created_at, updated_at
                 FROM notes WHERE id = '11111111-1111-4111-8111-111111111111'",
                [],
                |r| {
                    Ok((
                        r.get(0)?,
                        r.get(1)?,
                        r.get(2)?,
                        r.get(3)?,
                        r.get(4)?,
                        r.get(5)?,
                        r.get(6)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(note.0, "list");
        assert_eq!(note.1, "Grocery list");
        assert_eq!(note.2, "blue");
        assert_eq!(note.3, 1);
        assert_eq!(note.4, 1);
        assert_eq!(note.5, "2026-01-02T03:04:05Z");
        assert_eq!(note.6, "2026-01-03T03:04:05Z");
        let checklist_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM checklist_items
                 WHERE note_id = '11111111-1111-4111-8111-111111111111'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(checklist_count, 2);
        let label_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM labels", [], |r| r.get(0))
            .unwrap();
        assert_eq!(label_count, 2);
        let attachment: (String, String) = conn
            .query_row(
                "SELECT mime, resource_path FROM attachments
                 WHERE note_id = '11111111-1111-4111-8111-111111111111'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(attachment.0, "image/png");
        assert!(data_dir.join("resources").join(attachment.1).is_file());
    }

    #[test]
    fn import_markdown_vault_reports_id_collision_without_overwriting_existing_note() {
        let tmp = tempfile::tempdir().unwrap();
        let data_dir = tmp.path().join("data");
        let vault_dir = tmp.path().join("vault");
        std::fs::create_dir_all(&data_dir).unwrap();
        std::fs::create_dir_all(&vault_dir).unwrap();
        let mut conn = crate::db::open(&data_dir.join("keepr.db")).unwrap();
        conn.execute(
            "INSERT INTO notes (id, kind, title, body, color, pinned, archived, trashed, position, created_at, updated_at, background_pattern)
             VALUES ('11111111-1111-4111-8111-111111111111', 'text', 'Existing', '', 'default', 0, 0, 0, 0, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', '')",
            [],
        )
        .unwrap();
        std::fs::write(
            vault_dir.join("Imported.md"),
            concat!(
                "---\n",
                "id: 11111111-1111-4111-8111-111111111111\n",
                "type: text\n",
                "---\n",
                "# Imported\n",
                "\n",
                "New body\n"
            ),
        )
        .unwrap();

        let summary = import_markdown_vault_inner(&mut conn, &data_dir, &vault_dir).unwrap();

        assert_eq!(summary.notes_created, 1);
        assert_eq!(summary.collisions.len(), 1);
        assert!(summary.collisions[0].contains("existing note id"));
        let existing_title: String = conn
            .query_row(
                "SELECT title FROM notes WHERE id = '11111111-1111-4111-8111-111111111111'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(existing_title, "Existing");
        let imported_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM notes WHERE title = 'Imported'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(imported_count, 1);
    }

    #[test]
    fn map_keep_color_covers_full_enum() {
        for (k, v) in [
            ("RED", "red"),
            ("ORANGE", "orange"),
            ("YELLOW", "yellow"),
            ("GREEN", "green"),
            ("TEAL", "teal"),
            ("BLUE", "blue"),
            ("DARK_BLUE", "darkblue"),
            ("PURPLE", "purple"),
            ("PINK", "pink"),
            ("BROWN", "brown"),
            ("GRAY", "gray"),
            ("UNKNOWN", "default"),
            ("", "default"),
        ] {
            assert_eq!(map_keep_color(k), v, "for {k}");
        }
    }

    #[test]
    fn takeout_usec_to_rfc3339_round_trips() {
        // 2024-01-01T00:00:00Z = 1704067200 seconds = 1704067200_000_000 µs
        let usec = serde_json::json!(1704067200u64 * 1_000_000);
        let out = takeout_usec_to_rfc3339(Some(&usec)).unwrap();
        assert!(out.starts_with("2024-01-01T00:00:00"), "got: {out}");
        // Missing input → None
        assert_eq!(takeout_usec_to_rfc3339(None), None);
        // Non-number → None
        assert_eq!(
            takeout_usec_to_rfc3339(Some(&serde_json::json!("not a number"))),
            None
        );
    }

    #[test]
    fn takeout_reminder_fire_at_handles_multiple_shapes() {
        let fire_on = serde_json::json!({ "fireOn": "2024-06-15T08:00:00Z" });
        assert_eq!(
            takeout_reminder_fire_at(&fire_on),
            Some("2024-06-15T08:00:00Z".to_string())
        );
        let snake = serde_json::json!({ "fire_on": "2024-06-15T08:00:00Z" });
        assert_eq!(
            takeout_reminder_fire_at(&snake),
            Some("2024-06-15T08:00:00Z".to_string())
        );
        let usec = serde_json::json!({
            "reminderTimeUsec": 1718438400u64 * 1_000_000u64,
        });
        let result = takeout_reminder_fire_at(&usec).unwrap();
        assert!(result.starts_with("2024-06-15"), "got: {result}");
        let nested = serde_json::json!({
            "time": { "formattedDate": "2024-06-15T08:00:00Z" }
        });
        assert_eq!(
            takeout_reminder_fire_at(&nested),
            Some("2024-06-15T08:00:00Z".to_string())
        );
        let empty = serde_json::json!({});
        assert_eq!(takeout_reminder_fire_at(&empty), None);
        // Garbage timestamp → None
        let garbage = serde_json::json!({ "fireOn": "not-a-date" });
        assert_eq!(takeout_reminder_fire_at(&garbage), None);
    }

    #[test]
    fn is_keep_note_shape_accepts_canonical_takeout_note() {
        // Exact field set seen in a 2026 Google Takeout (Keep-only export).
        let note = serde_json::json!({
            "color": "DEFAULT",
            "isTrashed": false,
            "isPinned": true,
            "isArchived": false,
            "textContent": "Hello world",
            "title": "Test",
            "userEditedTimestampUsec": 1704067200000000u64,
            "createdTimestampUsec": 1704067200000000u64,
            "textContentHtml": "<p>Hello world</p>",
        });
        assert!(is_keep_note_shape(&note));
    }

    #[test]
    fn is_keep_note_shape_accepts_list_only_note() {
        // Checklist note without textContent — still a Keep note.
        let list = serde_json::json!({
            "isPinned": false,
            "isTrashed": false,
            "listContent": [{"text": "buy milk", "isChecked": false}],
            "createdTimestampUsec": 1u64,
        });
        assert!(is_keep_note_shape(&list));
    }

    #[test]
    fn is_keep_note_shape_rejects_takeout_labels_array() {
        // Takeout's `Labels.json` is a top-level array, not an object.
        let labels = serde_json::json!([{"name": "Work"}, {"name": "Personal"}]);
        assert!(!is_keep_note_shape(&labels));
    }

    #[test]
    fn is_keep_note_shape_rejects_other_product_json() {
        // Drive/Photos metadata in a multi-product Takeout — has no
        // `isPinned`, no Keep timestamps, no Keep content fields.
        let drive = serde_json::json!({
            "name": "Some doc",
            "lastModifiedTime": "2024-01-01T00:00:00Z",
            "mimeType": "application/pdf",
        });
        assert!(!is_keep_note_shape(&drive));
    }

    #[test]
    fn is_keep_note_shape_rejects_partial_match() {
        // Has `isPinned` but no content + no timestamps — not a note.
        let partial = serde_json::json!({"isPinned": false});
        assert!(!is_keep_note_shape(&partial));
        // Has content but no `isPinned` — also rejected.
        let no_pinned = serde_json::json!({
            "textContent": "x",
            "createdTimestampUsec": 1u64,
        });
        assert!(!is_keep_note_shape(&no_pinned));
    }

    #[test]
    fn guess_mime_for_ext_handles_known_and_unknown() {
        assert_eq!(guess_mime_for_ext("png"), "image/png");
        assert_eq!(guess_mime_for_ext("jpg"), "image/jpeg");
        assert_eq!(guess_mime_for_ext("jpeg"), "image/jpeg");
        assert_eq!(guess_mime_for_ext("gif"), "image/gif");
        assert_eq!(guess_mime_for_ext("webp"), "image/webp");
        assert_eq!(guess_mime_for_ext("svg"), "image/svg+xml");
        assert_eq!(guess_mime_for_ext("unknown"), "application/octet-stream");
    }

    #[test]
    fn content_addressed_paths_use_two_level_hash_fanout() {
        let hash = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";
        assert_eq!(
            content_addressed_rel_path(hash, "png"),
            format!("ab/cd/{hash}.png")
        );
        assert_eq!(
            content_addressed_thumb_path(hash),
            format!("ab/cd/{hash}.thumb.jpg")
        );
    }

    #[test]
    fn store_content_addressed_bytes_deduplicates_identical_payloads() {
        let tmp = tempfile::tempdir().unwrap();
        let first = store_content_addressed_bytes(tmp.path(), b"same image", "png").unwrap();
        let second = store_content_addressed_bytes(tmp.path(), b"same image", "png").unwrap();

        assert_eq!(first.rel_path, second.rel_path);
        assert!(first.created);
        assert!(!second.created);
        assert_eq!(
            std::fs::read(tmp.path().join(first.rel_path)).unwrap(),
            b"same image"
        );
    }

    // --- Direct-AppState integration tests ---
    //
    // These construct an AppState manually with an in-memory SQLite
    // connection so we can call commands' inner logic without going
    // through Tauri's State extractor. The commands wrapped in
    // #[tauri::command] still take State<'_, AppState>, so we duplicate
    // the body of the smaller ones into test-local helpers.

    fn test_state() -> AppState {
        use parking_lot::Mutex;
        use std::sync::atomic::AtomicBool;
        use std::sync::Arc;
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("test.db");
        let conn = crate::db::open(&db_path).unwrap();
        // Leak the tempdir so it lives for the test's lifetime; we never
        // delete it explicitly. Test processes are short-lived; OS cleans
        // up %TEMP% eventually.
        let data_dir = tmp.into_path();
        AppState {
            db: Arc::new(Mutex::new(conn)),
            importing: Arc::new(AtomicBool::new(false)),
            data_dir,
            vault_dek: Arc::new(Mutex::new(None)),
            shutdown: Arc::new(AtomicBool::new(false)),
            web_clipper: Arc::new(Mutex::new(crate::web_clipper::WebClipperInfo::default())),
        }
    }

    fn insert_test_note(state: &AppState, id: &str, title: &str) {
        let conn = state.db.lock();
        let now = "2026-01-01T00:00:00Z";
        conn.execute(
            "INSERT INTO notes (id, kind, title, body, color, pinned, archived, trashed, position, created_at, updated_at)
             VALUES (?1, 'text', ?2, '', 'default', 0, 0, 0, 0, ?3, ?3)",
            params![id, title, now],
        )
        .unwrap();
    }

    #[test]
    fn vault_note_withholds_plaintext_attachment_metadata() {
        let state = test_state();
        insert_test_note(&state, "vaulted", "secret");
        let conn = state.db.lock();
        conn.execute("UPDATE notes SET vault = 'vault' WHERE id = 'vaulted'", [])
            .unwrap();
        conn.execute(
            "INSERT INTO attachments (id, note_id, kind, mime, filename, byte_size, position, created_at, resource_path)
             VALUES ('a1', 'vaulted', 'image', 'image/png', 'secret.png', 4, 0, '2026-01-01T00:00:00Z', 'ab/cd/abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789.png')",
            [],
        )
        .unwrap();

        let note = load_note(&conn, "vaulted").unwrap().unwrap();

        assert_eq!(note.vault, "vault");
        assert_eq!(note.vault_attachment_count, 1);
        assert!(
            note.attachments.is_empty(),
            "vault notes must not expose plaintext resource paths"
        );
    }

    #[test]
    fn attachment_guard_rejects_vault_notes() {
        let state = test_state();
        insert_test_note(&state, "vaulted", "secret");
        let conn = state.db.lock();
        conn.execute("UPDATE notes SET vault = 'vault' WHERE id = 'vaulted'", [])
            .unwrap();

        let err = ensure_note_accepts_attachment(&conn, "vaulted").unwrap_err();

        assert!(err.contains("Private Vault notes cannot have attachments"));
    }

    #[test]
    fn resource_sweep_moves_unreferenced_files_to_trash() {
        let state = test_state();
        insert_test_note(&state, "n1", "has attachment");
        let resources = state.data_dir.join("resources");
        let referenced =
            "ab/cd/abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789.png";
        let orphan = "de/ad/deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef.png";
        let referenced_path = resources.join(referenced);
        let orphan_path = resources.join(orphan);
        std::fs::create_dir_all(referenced_path.parent().unwrap()).unwrap();
        std::fs::create_dir_all(orphan_path.parent().unwrap()).unwrap();
        std::fs::write(&referenced_path, b"kept").unwrap();
        std::fs::write(&orphan_path, b"moved").unwrap();

        {
            let conn = state.db.lock();
            conn.execute(
                "INSERT INTO attachments (id, note_id, kind, mime, filename, byte_size, position, created_at, resource_path)
                 VALUES ('a1', 'n1', 'image', 'image/png', 'kept.png', 4, 0, '2026-01-01T00:00:00Z', ?1)",
                params![referenced],
            )
            .unwrap();
            let stats = sweep_orphaned_resources_with_clock(
                &conn,
                &resources,
                std::time::SystemTime::now() + std::time::Duration::from_secs(1),
                std::time::Duration::ZERO,
                std::time::Duration::from_secs(30 * 24 * 60 * 60),
            )
            .unwrap();
            assert_eq!(stats.moved_to_trash, 1);
            assert_eq!(stats.purged, 0);
        }

        assert!(referenced_path.exists());
        assert!(!orphan_path.exists());
        assert!(resources.join(".trash").join(orphan).exists());
    }

    #[test]
    fn content_addressed_delete_keeps_shared_blob_until_last_reference() {
        let state = test_state();
        insert_test_note(&state, "n1", "first");
        insert_test_note(&state, "n2", "second");
        let resources = state.data_dir.join("resources");
        let rel = "ab/cd/abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789.png";
        let thumb =
            "ab/cd/abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789.thumb.jpg";
        let path = resources.join(rel);
        let thumb_path = resources.join(thumb);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"shared").unwrap();
        std::fs::write(&thumb_path, b"thumb").unwrap();

        let conn = state.db.lock();
        for (id, note_id) in [("a1", "n1"), ("a2", "n2")] {
            conn.execute(
                "INSERT INTO attachments (id, note_id, kind, mime, filename, byte_size, position, created_at, resource_path, thumb_path)
                 VALUES (?1, ?2, 'image', 'image/png', 'shared.png', 6, 0, '2026-01-01T00:00:00Z', ?3, ?4)",
                params![id, note_id, rel, thumb],
            )
            .unwrap();
        }

        conn.execute("DELETE FROM attachments WHERE id = 'a1'", [])
            .unwrap();
        delete_attachment_files(
            &conn,
            &resources,
            &AttachmentFiles {
                id: "a1".into(),
                mime: "image/png".into(),
                resource_path: Some(rel.into()),
                thumb_path: Some(thumb.into()),
            },
        );
        assert!(
            path.exists(),
            "shared resource deleted while a2 still references it"
        );
        assert!(
            thumb_path.exists(),
            "shared thumbnail deleted while a2 still references it"
        );

        conn.execute("DELETE FROM attachments WHERE id = 'a2'", [])
            .unwrap();
        delete_attachment_files(
            &conn,
            &resources,
            &AttachmentFiles {
                id: "a2".into(),
                mime: "image/png".into(),
                resource_path: Some(rel.into()),
                thumb_path: Some(thumb.into()),
            },
        );
        assert!(!path.exists(), "last reference should remove resource");
        assert!(
            !thumb_path.exists(),
            "last reference should remove shared thumbnail"
        );
    }

    #[test]
    fn peek_due_reminders_returns_only_pending_and_due() {
        let state = test_state();
        insert_test_note(&state, "n1", "due note");
        insert_test_note(&state, "n2", "future note");
        insert_test_note(&state, "n3", "fired note");
        let now = "2026-05-26T12:00:00Z";
        let conn = state.db.lock();
        conn.execute(
            "INSERT INTO reminders (note_id, fire_at, created_at) VALUES ('n1', '2026-05-26T11:00:00Z', ?1)",
            params![now],
        ).unwrap();
        conn.execute(
            "INSERT INTO reminders (note_id, fire_at, created_at) VALUES ('n2', '2026-05-26T13:00:00Z', ?1)",
            params![now],
        ).unwrap();
        conn.execute(
            "INSERT INTO reminders (note_id, fire_at, fired_at, created_at) VALUES ('n3', '2026-05-26T11:00:00Z', ?1, ?1)",
            params![now],
        ).unwrap();
        drop(conn);
        let due = peek_due_reminders(&state, now).unwrap();
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].0.note_id, "n1");
        assert_eq!(due[0].1, "due note");
    }

    #[test]
    fn peek_due_reminders_excludes_trashed_notes() {
        let state = test_state();
        insert_test_note(&state, "n_trash", "trashed");
        let conn = state.db.lock();
        conn.execute("UPDATE notes SET trashed = 1 WHERE id = 'n_trash'", [])
            .unwrap();
        conn.execute(
            "INSERT INTO reminders (note_id, fire_at, created_at) VALUES ('n_trash', '2026-05-26T11:00:00Z', '2026-05-26T11:00:00Z')",
            [],
        )
        .unwrap();
        drop(conn);
        let due = peek_due_reminders(&state, "2026-05-26T12:00:00Z").unwrap();
        assert_eq!(due.len(), 0);
    }

    #[test]
    fn mark_reminder_fired_sets_the_column() {
        let state = test_state();
        insert_test_note(&state, "n1", "x");
        let conn = state.db.lock();
        conn.execute(
            "INSERT INTO reminders (note_id, fire_at, created_at) VALUES ('n1', '2026-05-26T11:00:00Z', '2026-05-26T11:00:00Z')",
            [],
        )
        .unwrap();
        drop(conn);
        mark_reminder_fired(&state, "n1", "2026-05-26T12:00:00Z").unwrap();
        let conn = state.db.lock();
        let fired: Option<String> = conn
            .query_row(
                "SELECT fired_at FROM reminders WHERE note_id = 'n1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(fired.as_deref(), Some("2026-05-26T12:00:00Z"));
    }

    #[test]
    fn peek_does_not_write_fired_at() {
        // EI-V0.5-2 regression test: peek_due_reminders must NEVER mark
        // fired_at; only mark_reminder_fired does. Otherwise a failed
        // notification permanently loses the reminder.
        let state = test_state();
        insert_test_note(&state, "n1", "x");
        let conn = state.db.lock();
        conn.execute(
            "INSERT INTO reminders (note_id, fire_at, created_at) VALUES ('n1', '2026-05-26T11:00:00Z', '2026-05-26T11:00:00Z')",
            [],
        )
        .unwrap();
        drop(conn);
        let _ = peek_due_reminders(&state, "2026-05-26T12:00:00Z").unwrap();
        let _ = peek_due_reminders(&state, "2026-05-26T12:00:00Z").unwrap();
        let _ = peek_due_reminders(&state, "2026-05-26T12:00:00Z").unwrap();
        let conn = state.db.lock();
        let fired: Option<String> = conn
            .query_row(
                "SELECT fired_at FROM reminders WHERE note_id = 'n1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(fired.is_none(), "fired_at should still be NULL after peek");
    }

    #[test]
    fn format_ics_utc_rounds_offsets_to_zulu() {
        let out = format_ics_utc("2026-05-26T08:00:00-04:00").unwrap();
        assert_eq!(out, "20260526T120000Z");
    }

    #[test]
    fn build_fts5_query_quotes_and_prefixes_each_token() {
        // Plain text → phrase-quote per token, prefix wildcard, joined.
        assert_eq!(build_fts5_query("milk"), "\"milk\"*");
        assert_eq!(build_fts5_query("buy milk"), "\"buy\"* \"milk\"*");
        // Empty / whitespace-only → empty.
        assert_eq!(build_fts5_query(""), "");
        assert_eq!(build_fts5_query("   "), "");
        // FTS5-meaningful chars survive because the whole token is
        // wrapped in double quotes; embedded quotes are escaped.
        assert_eq!(build_fts5_query("foo(bar)"), "\"foo(bar)\"*");
        assert_eq!(build_fts5_query("ab\"cd"), "\"ab\"\"cd\"*");
        // AND / OR / NEAR — FTS5 keywords. Quoting neutralizes them.
        assert_eq!(
            build_fts5_query("milk OR eggs"),
            "\"milk\"* \"OR\"* \"eggs\"*"
        );
    }

    #[test]
    fn escape_ics_handles_special_characters() {
        // Backslash first so the order matters.
        let out = escape_ics("a\\b,c;d\ne");
        assert_eq!(out, "a\\\\b\\,c\\;d\\ne");
    }

    #[test]
    fn next_fire_at_handles_supported_rrules() {
        // Daily
        assert_eq!(
            next_fire_at("2026-05-26T08:00:00+00:00", Some("FREQ=DAILY"))
                .unwrap()
                .starts_with("2026-05-27T08:00:00"),
            true,
        );
        // Weekly
        assert_eq!(
            next_fire_at("2026-05-26T08:00:00+00:00", Some("FREQ=WEEKLY"))
                .unwrap()
                .starts_with("2026-06-02T08:00:00"),
            true,
        );
        // Monthly
        assert_eq!(
            next_fire_at("2026-05-26T08:00:00+00:00", Some("FREQ=MONTHLY"))
                .unwrap()
                .starts_with("2026-06-26T08:00:00"),
            true,
        );
        // Yearly (with leap-day clamp)
        assert!(
            next_fire_at("2024-02-29T08:00:00+00:00", Some("FREQ=YEARLY"))
                .unwrap()
                .starts_with("2025-02-28")
        );
        // None for single-shot
        assert_eq!(next_fire_at("2026-05-26T08:00:00+00:00", None), None);
        // Unsupported rrule
        assert_eq!(
            next_fire_at("2026-05-26T08:00:00+00:00", Some("FREQ=HOURLY")),
            None
        );
    }

    #[test]
    fn validate_rrule_rejects_unknown() {
        assert!(validate_rrule(None).is_ok());
        assert!(validate_rrule(Some("FREQ=DAILY")).is_ok());
        assert!(validate_rrule(Some("FREQ=WEEKLY")).is_ok());
        assert!(validate_rrule(Some("FREQ=MONTHLY")).is_ok());
        assert!(validate_rrule(Some("FREQ=YEARLY")).is_ok());
        assert!(validate_rrule(Some("FREQ=HOURLY")).is_err());
        assert!(validate_rrule(Some("garbage")).is_err());
    }

    #[test]
    fn mark_reminder_fired_advances_recurring() {
        // NF-V0.5-A regression test — recurring reminders re-arm to the
        // next occurrence after a successful fire; fired_at stays NULL
        // so the row is still pending in the next sweep.
        let state = test_state();
        insert_test_note(&state, "n1", "daily standup");
        let conn = state.db.lock();
        conn.execute(
            "INSERT INTO reminders (note_id, fire_at, rrule, created_at) VALUES ('n1', '2026-05-26T08:00:00+00:00', 'FREQ=DAILY', '2026-05-26T08:00:00+00:00')",
            [],
        )
        .unwrap();
        drop(conn);
        mark_reminder_fired(&state, "n1", "2026-05-26T08:05:00+00:00").unwrap();
        let conn = state.db.lock();
        let (fire_at, fired_at): (String, Option<String>) = conn
            .query_row(
                "SELECT fire_at, fired_at FROM reminders WHERE note_id = 'n1'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert!(
            fire_at.starts_with("2026-05-27T08:00:00"),
            "got fire_at: {fire_at}"
        );
        assert!(
            fired_at.is_none(),
            "fired_at should be NULL after recurring advance"
        );
    }

    #[test]
    fn mark_reminder_fired_single_shot_sets_fired_at() {
        let state = test_state();
        insert_test_note(&state, "n1", "single");
        let conn = state.db.lock();
        conn.execute(
            "INSERT INTO reminders (note_id, fire_at, created_at) VALUES ('n1', '2026-05-26T08:00:00+00:00', '2026-05-26T08:00:00+00:00')",
            [],
        )
        .unwrap();
        drop(conn);
        mark_reminder_fired(&state, "n1", "2026-05-26T08:05:00+00:00").unwrap();
        let conn = state.db.lock();
        let fired_at: Option<String> = conn
            .query_row(
                "SELECT fired_at FROM reminders WHERE note_id = 'n1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(fired_at.is_some(), "fired_at should be set for single-shot");
    }

    #[test]
    fn migration_v4_backfills_positions() {
        // EI-V0.5-1 regression test: every note after a fresh migrate
        // should have a unique position (0..N-1).
        let state = test_state();
        insert_test_note(&state, "a", "first");
        insert_test_note(&state, "b", "second");
        insert_test_note(&state, "c", "third");
        // Mutate updated_at so the ROW_NUMBER OVER (ORDER BY updated_at DESC) sees
        // a deterministic order.
        let conn = state.db.lock();
        conn.execute(
            "UPDATE notes SET updated_at = '2026-05-01' WHERE id = 'a'",
            [],
        )
        .unwrap();
        conn.execute(
            "UPDATE notes SET updated_at = '2026-05-03' WHERE id = 'b'",
            [],
        )
        .unwrap();
        conn.execute(
            "UPDATE notes SET updated_at = '2026-05-02' WHERE id = 'c'",
            [],
        )
        .unwrap();
        // Re-run the v4 migration body directly.
        conn.execute_batch(
            "WITH ordered AS (
                SELECT id,
                       ROW_NUMBER() OVER (ORDER BY pinned DESC, updated_at DESC) - 1 AS rn
                FROM notes
            )
            UPDATE notes
            SET position = (SELECT rn FROM ordered WHERE ordered.id = notes.id);",
        )
        .unwrap();
        let mut stmt = conn
            .prepare("SELECT id, position FROM notes ORDER BY position ASC")
            .unwrap();
        let rows: Vec<(String, i64)> = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].1, 0);
        assert_eq!(rows[1].1, 1);
        assert_eq!(rows[2].1, 2);
        // First (most recent) should be b.
        assert_eq!(rows[0].0, "b");
    }
}
