import type { Note } from "../types";

const MS_PER_DAY = 24 * 60 * 60 * 1000;

/**
 * Days remaining until a note in Trash would be auto-purged. Returns null
 * when the note isn't trashed, retention is disabled, or trashed_at is
 * missing. Returns 0 when the note is already past its retention window
 * (the caller should treat this as "purge on next sweep").
 */
export function daysLeftInTrash(
  note: Note,
  retentionDays: number,
  now: number = Date.now(),
): number | null {
  if (!note.trashed || retentionDays <= 0 || !note.trashed_at) return null;
  const trashedAt = Date.parse(note.trashed_at);
  if (Number.isNaN(trashedAt)) return null;
  const expiresAt = trashedAt + retentionDays * MS_PER_DAY;
  const msLeft = expiresAt - now;
  if (msLeft <= 0) return 0;
  return Math.max(1, Math.ceil(msLeft / MS_PER_DAY));
}

/** Return every trashed note that is past its retention window (NF-17). */
export function findExpiredTrashed(
  notes: Note[],
  retentionDays: number,
  now: number = Date.now(),
): Note[] {
  if (retentionDays <= 0) return [];
  return notes.filter((n) => daysLeftInTrash(n, retentionDays, now) === 0);
}
