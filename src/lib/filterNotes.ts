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
  /** EI-18 — when set, narrow the post-section/filter pool to these IDs.
   *  Populated by the TopBar's debounced `api.searchNotes(query)` call.
   *  When `null`/`undefined`, falls back to the in-memory substring scan
   *  (used in tests and in the browser-preview build that has no Tauri
   *  backend). The substring scan is also a defensive fallback if the
   *  FTS5 call errors. */
  searchMatchIds?: Set<string> | null,
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
    if (filters.hasImage) {
      pool = pool.filter((n) => (n.attachments?.length ?? 0) > 0);
    }
    if (filters.hasReminder) {
      const active = new Set(
        (reminders ?? []).filter(isActive).map((r) => r.noteId),
      );
      pool = pool.filter((n) => active.has(n.id));
    }
    if (filters.inVault) {
      pool = pool.filter((n) => n.vault === "vault");
    }
  }

  const q = search.trim().toLowerCase();
  if (!q) return pool;
  // EI-18 — prefer the FTS5-backed Set when available. Substring scan
  // is the fallback for tests / browser preview / Rust-side errors.
  if (searchMatchIds) {
    return pool.filter((n) => searchMatchIds.has(n.id));
  }
  return pool.filter((n) => {
    if (n.title.toLowerCase().includes(q)) return true;
    if (n.body.toLowerCase().includes(q)) return true;
    if (n.checklist.some((c) => c.text.toLowerCase().includes(q))) return true;
    return false;
  });
}
