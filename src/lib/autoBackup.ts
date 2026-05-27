import type { AutoBackupCadence } from "../store";

const MS_PER_DAY = 24 * 60 * 60 * 1000;

export function cadenceMs(cadence: AutoBackupCadence): number {
  if (cadence === "daily") return MS_PER_DAY;
  if (cadence === "weekly") return 7 * MS_PER_DAY;
  return Infinity;
}

/**
 * Should we run an auto-backup right now? True when the cadence is enabled,
 * a folder is configured, and either no backup has ever run or the elapsed
 * time since the last one exceeds the cadence window. NF-15.
 */
export function isBackupDue(
  cadence: AutoBackupCadence,
  folder: string | null,
  lastAt: string | null,
  now: number = Date.now(),
): boolean {
  if (cadence === "off") return false;
  if (!folder) return false;
  if (!lastAt) return true;
  const last = Date.parse(lastAt);
  if (Number.isNaN(last)) return true;
  return now - last >= cadenceMs(cadence);
}

/** Build the auto-backup file name for `now` — `keepr-autobackup-<ISO>.zip`. */
export function backupFilename(now: Date = new Date()): string {
  const stamp = now.toISOString().replace(/[:.]/g, "-").slice(0, 19);
  return `keepr-autobackup-${stamp}.zip`;
}

/** Join the folder + filename with the platform's path separator. The Rust
 *  side is happy with forward slashes on Windows, so we keep it simple. */
export function backupPath(folder: string, filename: string): string {
  // Trim trailing separators so we don't double them.
  const trimmed = folder.replace(/[\\/]+$/, "");
  return `${trimmed}/${filename}`;
}

/** v0.21.0 — given a list of `keepr-autobackup-<ISO>.zip` filenames,
 *  return the ones that should be deleted to enforce a "keep latest N
 *  by sort order" retention. The ISO stamp in the filename sorts
 *  lexicographically as it does chronologically, so a string sort is
 *  enough. Returns filenames (not full paths). Pure for testing. */
export function backupsToPrune(filenames: string[], keep: number): string[] {
  if (keep <= 0) return [];
  const ours = filenames
    .filter((f) => f.startsWith("keepr-autobackup-") && f.endsWith(".zip"))
    .slice()
    .sort();
  if (ours.length <= keep) return [];
  // Newest are last (ISO ascending). Drop everything BEFORE the last `keep`.
  return ours.slice(0, ours.length - keep);
}
