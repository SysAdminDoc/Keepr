/**
 * v0.22.3 — `[[Note Title]]` two-way note linking. Renderer-only:
 * no schema, no Tauri command. Computes "mentions" and "backlinks"
 * on-the-fly from the in-memory note bodies. For typical note counts
 * (<2000) the O(N * mentions-per-note) cost is well under a frame.
 *
 * Title resolution is case-insensitive on the full title trim. Title
 * collisions (two notes with the same title) all resolve as candidates;
 * the renderer shows whichever it finds first.
 */

import type { Note } from "../types";

const LINK_REGEX = /\[\[([^\]\n]+?)\]\]/g;

/** Extract every `[[Title]]` mention from a body. Returns unique titles
 *  in document order. Leading/trailing whitespace inside the brackets
 *  is trimmed. Empty `[[]]` is skipped. */
export function extractWikiLinks(body: string): string[] {
  if (!body) return [];
  const seen = new Set<string>();
  const out: string[] = [];
  const re = new RegExp(LINK_REGEX.source, "g");
  let m: RegExpExecArray | null;
  while ((m = re.exec(body)) !== null) {
    const t = m[1].trim();
    if (!t) continue;
    if (seen.has(t.toLowerCase())) continue;
    seen.add(t.toLowerCase());
    out.push(t);
  }
  return out;
}

/** Resolve a title to a Note in the pool, case-insensitive. Returns
 *  the first match (undefined if none). Trashed notes are excluded so
 *  a deleted target doesn't show as a clickable chip. */
export function resolveTitle(notes: Note[], title: string): Note | undefined {
  const target = title.trim().toLowerCase();
  return notes.find((n) => !n.trashed && n.title.trim().toLowerCase() === target);
}

/** Find every note that mentions `targetTitle` (case-insensitive) in
 *  its body via `[[Title]]`. Excludes the note with id `excludeId` so
 *  the editor doesn't list a note as a backlink to itself if a user
 *  happens to write [[Own Title]] inside it. */
export function findBacklinks(
  notes: Note[],
  targetTitle: string,
  excludeId: string,
): Note[] {
  const t = targetTitle.trim().toLowerCase();
  if (!t) return [];
  const out: Note[] = [];
  for (const n of notes) {
    if (n.id === excludeId) continue;
    if (n.trashed) continue;
    const links = extractWikiLinks(n.body);
    if (links.some((l) => l.toLowerCase() === t)) out.push(n);
  }
  return out;
}
