/**
 * NF-07 — parse `#hashtag` tokens out of note title + body and turn them
 * into labels. Mirrors Memos's UX: typing `#groceries` in a note body
 * is equivalent to picking the "groceries" label from the menu.
 *
 * Hashtag grammar:
 *   #<letter-or-digit-or-underscore-or-hyphen>+   (Unicode letters/numbers)
 * Must start with `#` and either be at the start of input or preceded by
 * whitespace, so URL fragments like `#section-id` inside an `https://…/#x`
 * aren't picked up. Pure-numeric tags (`#1`, `#42`) are excluded —
 * recipe-step / numbered-item false-positives.
 */

const HASHTAG_REGEX = /(?:^|\s)#([\p{L}_][\p{L}\p{N}_-]*)/gu;

/** Extract every unique hashtag (case-insensitive) from a string,
 *  in first-appearance order. */
export function extractHashtags(text: string): string[] {
  if (!text) return [];
  const seen = new Set<string>();
  const out: string[] = [];
  for (const m of text.matchAll(HASHTAG_REGEX)) {
    const tag = m[1];
    const key = tag.toLowerCase();
    if (!seen.has(key)) {
      seen.add(key);
      out.push(tag);
    }
  }
  return out;
}

/** Collect hashtags from a note's title, body, and every checklist item. */
export function extractHashtagsFromNote(parts: {
  title: string;
  body: string;
  checklist: { text: string }[];
}): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  const collect = (text: string) => {
    for (const tag of extractHashtags(text)) {
      const key = tag.toLowerCase();
      if (!seen.has(key)) {
        seen.add(key);
        out.push(tag);
      }
    }
  };
  collect(parts.title);
  collect(parts.body);
  for (const item of parts.checklist) collect(item.text);
  return out;
}
