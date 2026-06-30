//! Local end-to-end smoke verifier for Keepr's storage and clipper paths.
//!
//! This intentionally avoids the GUI so it can run unattended on the
//! developer machine. It still uses the real schema migrations, vault
//! crypto, content-addressed attachment layout, backup ZIP shape, and
//! Web Clipper localhost server.

use anyhow::{anyhow, bail, Context, Result};
use parking_lot::Mutex;
use rusqlite::{params, Connection};
use serde_json::json;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;
use walkdir::WalkDir;
use zip::write::SimpleFileOptions;

const SMOKE_PASSWORD: &str = "Keepr smoke vault password";
const ONE_BY_ONE_PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xf8, 0xcf, 0xc0, 0xf0,
    0x1f, 0x00, 0x05, 0x00, 0x01, 0xff, 0xa7, 0x69, 0x6c, 0x6d, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
    0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

#[derive(Debug)]
struct Args {
    keep: bool,
    help: bool,
}

#[derive(Debug)]
struct SmokePaths {
    root: PathBuf,
    data_dir: PathBuf,
    restore_dir: PathBuf,
    backup_zip: PathBuf,
}

fn main() {
    if let Err(e) = run() {
        eprintln!("keepr-smoke: {e:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let args = parse_args()?;
    if args.help {
        print_help();
        return Ok(());
    }

    let paths = SmokePaths::new()?;
    fs::create_dir_all(&paths.data_dir)?;
    fs::create_dir_all(paths.data_dir.join("resources"))?;
    fs::create_dir_all(&paths.restore_dir)?;

    let db_path = paths.data_dir.join("keepr.db");
    let conn = keepr_lib::db::open(&db_path).context("open smoke database")?;
    assert_schema(&conn)?;
    println!("OK schema: migrated temp database to current schema");

    let note_id = create_note(&conn)?;
    let resource_rel = attach_image(&conn, &paths.data_dir, &note_id)?;
    assert_attachment_resource(&paths.data_dir, &resource_rel)?;
    println!("OK attachment: image row and content-addressed resource written");

    let vault_note_id = exercise_vault(&conn)?;
    println!("OK vault: initialized, encrypted, locked, unlocked, and decrypted {vault_note_id}");

    let db = Arc::new(Mutex::new(conn));
    let clip_note_id = exercise_web_clipper(db.clone())?;
    println!("OK clipper: localhost bearer POST inserted note {clip_note_id}");

    {
        let conn = db.lock();
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .context("checkpoint smoke database before backup")?;
    }
    export_backup(&paths.data_dir, &paths.backup_zip)?;
    restore_backup(&paths.backup_zip, &paths.restore_dir)?;
    assert_restored_backup(&paths.restore_dir, &resource_rel, &clip_note_id)?;
    println!("OK backup: exported and restored keepr.db plus resources");

    if args.keep {
        println!("OK smoke complete; kept {}", paths.root.display());
    } else {
        drop(db);
        fs::remove_dir_all(&paths.root)
            .with_context(|| format!("remove temp smoke dir {}", paths.root.display()))?;
        println!("OK smoke complete; temp data removed");
    }
    Ok(())
}

fn parse_args() -> Result<Args> {
    let mut keep = false;
    let mut help = false;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--keep" => keep = true,
            "--help" | "-h" => help = true,
            other => bail!("unknown argument: {other}"),
        }
    }
    Ok(Args { keep, help })
}

fn print_help() {
    println!(
        "keepr-smoke - local storage and Web Clipper smoke verifier\n\
         \n\
         Usage:\n\
           keepr-smoke [--keep]\n\
         \n\
         Creates a temp Keepr data directory, migrates a database,\n\
         inserts a note, writes a content-addressed image attachment,\n\
         exercises vault encrypt/lock/unlock/decrypt behavior, posts a\n\
         clip through the real localhost Web Clipper server, exports a\n\
         backup ZIP, restores it into a second temp folder, and verifies\n\
         the restored DB/resources. --keep preserves the temp directory."
    );
}

impl SmokePaths {
    fn new() -> Result<Self> {
        let root = std::env::temp_dir().join(format!("keepr-smoke-{}", Uuid::new_v4()));
        Ok(Self {
            data_dir: root.join("data"),
            restore_dir: root.join("restore"),
            backup_zip: root.join("keepr-smoke-backup.zip"),
            root,
        })
    }
}

fn assert_schema(conn: &Connection) -> Result<()> {
    let version: i32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if version != keepr_lib::db::SCHEMA_VERSION {
        bail!(
            "schema version mismatch: got {version}, expected {}",
            keepr_lib::db::SCHEMA_VERSION
        );
    }
    Ok(())
}

fn create_note(conn: &Connection) -> Result<String> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO notes (id, kind, title, body, color, pinned, archived, trashed,
                            position, created_at, updated_at, background_pattern)
         VALUES (?1, 'text', 'Smoke note', 'Created by keepr-smoke', 'default', 0, 0, 0,
                 0, ?2, ?2, '')",
        params![id, now],
    )?;
    Ok(id)
}

fn attach_image(conn: &Connection, data_dir: &Path, note_id: &str) -> Result<String> {
    let resources_dir = data_dir.join("resources");
    let hash = blake3::hash(ONE_BY_ONE_PNG).to_hex().to_string();
    let rel = format!("{}/{}/{}.png", &hash[0..2], &hash[2..4], hash);
    let target = resources_dir.join(&rel);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&target, ONE_BY_ONE_PNG)?;
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO attachments (id, note_id, kind, mime, filename, byte_size,
                                  width, height, position, created_at, resource_path)
         VALUES (?1, ?2, 'image', 'image/png', 'smoke.png', ?3, 1, 1, 0, ?4, ?5)",
        params![
            Uuid::new_v4().to_string(),
            note_id,
            ONE_BY_ONE_PNG.len() as i64,
            now,
            rel
        ],
    )?;
    Ok(rel)
}

fn assert_attachment_resource(data_dir: &Path, rel: &str) -> Result<()> {
    let path = data_dir.join("resources").join(rel);
    let meta =
        fs::metadata(&path).with_context(|| format!("missing resource {}", path.display()))?;
    if meta.len() != ONE_BY_ONE_PNG.len() as u64 {
        bail!("resource size mismatch for {}", path.display());
    }
    Ok(())
}

fn exercise_vault(conn: &Connection) -> Result<String> {
    let (init, dek) = keepr_lib::vault::init(SMOKE_PASSWORD)?;
    conn.execute(
        "INSERT INTO app_settings(key, value) VALUES
            ('vault_kdf_salt', ?1),
            ('vault_dek_nonce', ?2),
            ('vault_dek_wrapped', ?3)",
        params![
            keepr_lib::vault::to_hex(&init.salt),
            keepr_lib::vault::to_hex(&init.dek_nonce),
            keepr_lib::vault::to_hex(&init.dek_wrapped),
        ],
    )?;

    let note_id = Uuid::new_v4().to_string();
    let payload = keepr_lib::vault::VaultPayload {
        title: "Smoke vault note".into(),
        body: "Vault body survives lock/unlock.".into(),
        checklist: vec![],
    };
    let encrypted = keepr_lib::vault::encrypt_note(&dek, &note_id, &payload)?;
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO notes (id, kind, title, body, color, pinned, archived, trashed,
                            position, created_at, updated_at, vault, vault_ciphertext,
                            background_pattern)
         VALUES (?1, 'text', '', '', 'default', 0, 0, 0, 1, ?2, ?2, 'vault', ?3, '')",
        params![note_id, now, keepr_lib::vault::to_hex(&encrypted)],
    )?;
    drop(dek);

    let locked_title: String = conn.query_row(
        "SELECT title FROM notes WHERE id = ?1",
        params![note_id],
        |r| r.get(0),
    )?;
    if !locked_title.is_empty() {
        bail!("vault note leaked plaintext title while locked");
    }

    let unlocked = keepr_lib::vault::unlock(
        SMOKE_PASSWORD,
        &init.salt,
        &init.dek_nonce,
        &init.dek_wrapped,
    )?
    .ok_or_else(|| anyhow!("vault unlock returned wrong-password state"))?;
    let decrypted = keepr_lib::vault::decrypt_note(&unlocked, &note_id, &encrypted)?;
    if decrypted.title != payload.title || decrypted.body != payload.body {
        bail!("vault decrypt payload mismatch");
    }
    Ok(note_id)
}

fn exercise_web_clipper(db: Arc<Mutex<Connection>>) -> Result<String> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_io()
        .enable_time()
        .build()
        .context("build tokio runtime")?;
    runtime.block_on(async move {
        let info: keepr_lib::web_clipper::WebClipperState =
            Arc::new(Mutex::new(keepr_lib::web_clipper::WebClipperInfo::default()));
        let port = keepr_lib::web_clipper::start_server(db.clone(), info.clone())
            .await
            .map_err(anyhow::Error::msg)?;
        let token = info
            .lock()
            .token
            .clone()
            .ok_or_else(|| anyhow!("web clipper token missing after server start"))?;
        let client = reqwest::Client::new();
        let health = client
            .get(format!("http://127.0.0.1:{port}/health"))
            .send()
            .await
            .context("GET /health")?;
        if !health.status().is_success() {
            bail!("web clipper health returned {}", health.status());
        }
        let payload = json!({
            "url": "https://keepr.invalid/smoke",
            "title": "Smoke clip",
            "markdown": "# Smoke clip\n\nSaved by keepr-smoke.",
            "excerpt": "Smoke excerpt",
            "tags": ["clipped", "smoke"]
        });
        let clip = client
            .post(format!("http://127.0.0.1:{port}/clip"))
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {token}"))
            .body(payload.to_string())
            .send()
            .await
            .context("POST /clip")?;
        if !clip.status().is_success() {
            bail!("web clipper POST returned {}", clip.status());
        }
        let note_id: String = {
            let conn = db.lock();
            conn.query_row(
                "SELECT id FROM notes WHERE title = 'Smoke clip' ORDER BY created_at DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .context("find smoke clip note")?
        };
        Ok(note_id)
    })
}

fn export_backup(data_dir: &Path, dest: &Path) -> Result<()> {
    let file = File::create(dest).with_context(|| format!("create {}", dest.display()))?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    zip.start_file("keepr.db", opts)?;
    let mut db_file = File::open(data_dir.join("keepr.db"))?;
    std::io::copy(&mut db_file, &mut zip)?;

    let resources = data_dir.join("resources");
    if resources.exists() {
        for entry in WalkDir::new(&resources).into_iter().filter_map(Result::ok) {
            if !entry.file_type().is_file() {
                continue;
            }
            let rel = entry
                .path()
                .strip_prefix(data_dir)
                .context("strip data dir prefix")?
                .to_string_lossy()
                .replace('\\', "/");
            zip.start_file(rel, opts)?;
            let mut input = File::open(entry.path())?;
            std::io::copy(&mut input, &mut zip)?;
        }
    }
    zip.finish()?.flush()?;
    Ok(())
}

fn restore_backup(src: &Path, dest_dir: &Path) -> Result<()> {
    let file = File::open(src).with_context(|| format!("open {}", src.display()))?;
    let mut archive = zip::ZipArchive::new(file)?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let Some(rel) = entry.enclosed_name().map(|p| p.to_path_buf()) else {
            bail!("unsafe backup entry: {}", entry.name());
        };
        let out = dest_dir.join(rel);
        if entry.is_dir() {
            fs::create_dir_all(&out)?;
            continue;
        }
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut output = File::create(&out)?;
        std::io::copy(&mut entry, &mut output)?;
    }
    Ok(())
}

fn assert_restored_backup(
    restore_dir: &Path,
    resource_rel: &str,
    clip_note_id: &str,
) -> Result<()> {
    let restored_db = restore_dir.join("keepr.db");
    let conn = keepr_lib::db::open(&restored_db).context("open restored DB")?;
    let clip_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM notes WHERE id = ?1 AND title = 'Smoke clip'",
        params![clip_note_id],
        |r| r.get(0),
    )?;
    if clip_count != 1 {
        bail!("restored backup missing smoke clip note");
    }
    let restored_resource = restore_dir.join("resources").join(resource_rel);
    if !restored_resource.is_file() {
        bail!("restored backup missing {}", restored_resource.display());
    }
    Ok(())
}
