import type { Note, Section } from "../types";

/**
 * Filter notes by section (Notes / Archive / Trash / Label) and an optional
 * substring search. Pure function — easy to unit-test.
 *
 * - Notes section: excludes archived AND trashed.
 * - Archive: archived AND NOT trashed.
 * - Trash: trashed (regardless of archived).
 * - Label: not trashed AND has the label.
 * Search is case-insensitive across title, body, and checklist item text.
 */
export function filterNotes(
  notes: Note[],
  section: Section,
  search: string,
): Note[] {
  let pool = notes;
  if (section.kind === "notes") {
    pool = pool.filter((n) => !n.archived && !n.trashed);
  } else if (section.kind === "archive") {
    pool = pool.filter((n) => n.archived && !n.trashed);
  } else if (section.kind === "trash") {
    pool = pool.filter((n) => n.trashed);
  } else if (section.kind === "label") {
    pool = pool.filter((n) => !n.trashed && n.labels.includes(section.labelId));
  }
  const q = search.trim().toLowerCase();
  if (!q) return pool;
  return pool.filter((n) => {
    if (n.title.toLowerCase().includes(q)) return true;
    if (n.body.toLowerCase().includes(q)) return true;
    if (n.checklist.some((c) => c.text.toLowerCase().includes(q))) return true;
    return false;
  });
}
