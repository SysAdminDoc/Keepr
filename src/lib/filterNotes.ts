import type { Note, Reminder, SearchFilters, Section } from "../types";
import { compareByDue, isActive } from "./reminders";

/**
 * Filter notes by section (Notes / Archive / Trash / Label), an optional
 * substring search, and an optional set of facet filters from the
 * NF-09 chip row. Pure function — easy to unit-test.
 *
 * Section rules:
 * - Notes: excludes archived AND trashed.
 * - Archive: archived AND NOT trashed.
 * - Trash: trashed (regardless of archived).
 * - Label: not trashed AND has the label.
 *
 * Filter rules (NF-09):
 * - Within a facet (kinds / colors / labelIds): OR.
 * - Across facets: AND.
 * - Empty facet = no constraint from that facet.
 *
 * Search is case-insensitive across title, body, and checklist item text.
 */
export function filterNotes(
  notes: Note[],
  section: Section,
  search: string,
  filters?: SearchFilters,
  reminders?: Reminder[],
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
  } else if (section.kind === "reminders") {
    // Order notes by when their reminder is next due. Excludes trashed
    // notes (a trashed reminder is dead weight) and notes without an
    // active reminder.
    const due = new Map<string, Reminder>();
    for (const r of reminders ?? []) {
      if (!isActive(r)) continue;
      due.set(r.noteId, r);
    }
    pool = pool
      .filter((n) => !n.trashed && due.has(n.id))
      .sort((a, b) => compareByDue(due.get(a.id)!, due.get(b.id)!));
  }

  if (filters) {
    if (filters.kinds.length > 0) {
      pool = pool.filter((n) => filters.kinds.includes(n.kind));
    }
    if (filters.colors.length > 0) {
      pool = pool.filter((n) => filters.colors.includes(n.color));
    }
    if (filters.labelIds.length > 0) {
      pool = pool.filter((n) =>
        filters.labelIds.some((id) => n.labels.includes(id)),
      );
    }
    if (filters.pinnedOnly) {
      pool = pool.filter((n) => n.pinned);
    }
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
