import type { BackgroundPatternKey } from "../types";

/** NF-22 — the nine Keep-shaped background patterns plus "none". Each
 *  value is a CSS `background-image` payload (data: URI SVG) tiled at
 *  the natural 80–120 px size. Patterns are deliberately low-contrast
 *  so card text stays legible without an opacity overlay; in dark mode
 *  the same SVGs render against the card's dark background and read as
 *  a subtle wash. */
export const BACKGROUND_PATTERNS: Record<BackgroundPatternKey, string> = {
  "": "",
  groceries: bagSvg("#8a6d3b"),
  food: forkSvg("#a05a2c"),
  music: noteSvg("#5b3a8c"),
  recipes: bookSvg("#5d7a3a"),
  notes: linesSvg("#777"),
  places: pinSvg("#b03a3a"),
  travel: planeSvg("#3a6b9a"),
  video: clapSvg("#444"),
  celebration: confettiSvg(),
};

export const BACKGROUND_PATTERN_LABELS: Record<BackgroundPatternKey, string> = {
  "": "None",
  groceries: "Groceries",
  food: "Food",
  music: "Music",
  recipes: "Recipes",
  notes: "Notes",
  places: "Places",
  travel: "Travel",
  video: "Video",
  celebration: "Celebration",
};

/** Order used in the picker — matches Keep web. "" first so the user
 *  always has an obvious "remove" affordance. */
export const BACKGROUND_PATTERN_ORDER: BackgroundPatternKey[] = [
  "",
  "groceries",
  "food",
  "music",
  "recipes",
  "notes",
  "places",
  "travel",
  "video",
  "celebration",
];

function svgUrl(svg: string): string {
  // Inline SVG → data URI. `encodeURIComponent` keeps the URL legal in
  // a CSS `url(...)` expression without needing a helper at every callsite.
  return `url("data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}")`;
}

function bagSvg(c: string): string {
  return svgUrl(
    `<svg xmlns='http://www.w3.org/2000/svg' width='90' height='90' viewBox='0 0 90 90' fill='none' opacity='0.16'>
       <g stroke='${c}' stroke-width='2'>
         <rect x='20' y='30' width='50' height='45' rx='3'/>
         <path d='M30 30 v-6 a15 15 0 0 1 30 0 v6'/>
       </g>
     </svg>`,
  );
}

function forkSvg(c: string): string {
  return svgUrl(
    `<svg xmlns='http://www.w3.org/2000/svg' width='90' height='90' viewBox='0 0 90 90' fill='none' opacity='0.16'>
       <g stroke='${c}' stroke-width='2' stroke-linecap='round'>
         <path d='M25 20 v25 a8 8 0 0 0 16 0 v-25'/>
         <path d='M33 45 v25'/>
         <path d='M60 20 c5 5 5 25 0 30 v20'/>
       </g>
     </svg>`,
  );
}

function noteSvg(c: string): string {
  return svgUrl(
    `<svg xmlns='http://www.w3.org/2000/svg' width='90' height='90' viewBox='0 0 90 90' fill='${c}' opacity='0.16'>
       <path d='M30 65 a6 6 0 1 0 12 0 v-30 l18-4 v25 a6 6 0 1 0 12 0 v-40 l-42 8 z'/>
     </svg>`,
  );
}

function bookSvg(c: string): string {
  return svgUrl(
    `<svg xmlns='http://www.w3.org/2000/svg' width='90' height='90' viewBox='0 0 90 90' fill='none' opacity='0.16'>
       <g stroke='${c}' stroke-width='2'>
         <path d='M15 25 q15 -5 30 0 q15 -5 30 0 v40 q-15 -5 -30 0 q-15 -5 -30 0 z'/>
         <line x1='45' y1='25' x2='45' y2='65'/>
       </g>
     </svg>`,
  );
}

function linesSvg(c: string): string {
  return svgUrl(
    `<svg xmlns='http://www.w3.org/2000/svg' width='90' height='30' viewBox='0 0 90 30' fill='none' opacity='0.13'>
       <g stroke='${c}' stroke-width='1.5'>
         <line x1='10' y1='10' x2='80' y2='10'/>
         <line x1='10' y1='20' x2='65' y2='20'/>
       </g>
     </svg>`,
  );
}

function pinSvg(c: string): string {
  return svgUrl(
    `<svg xmlns='http://www.w3.org/2000/svg' width='90' height='90' viewBox='0 0 90 90' fill='${c}' opacity='0.16'>
       <path d='M45 15 a15 15 0 0 1 15 15 c0 12 -15 30 -15 30 s-15 -18 -15 -30 a15 15 0 0 1 15 -15 z M45 25 a5 5 0 1 0 0 10 a5 5 0 0 0 0 -10 z'/>
     </svg>`,
  );
}

function planeSvg(c: string): string {
  return svgUrl(
    `<svg xmlns='http://www.w3.org/2000/svg' width='90' height='90' viewBox='0 0 90 90' fill='${c}' opacity='0.16'>
       <path d='M70 20 l-10 30 l15 5 l-3 8 l-22 -6 l-14 14 l-6 -2 l6 -18 l-22 -6 l3 -8 l30 4 z'/>
     </svg>`,
  );
}

function clapSvg(c: string): string {
  return svgUrl(
    `<svg xmlns='http://www.w3.org/2000/svg' width='90' height='90' viewBox='0 0 90 90' fill='none' opacity='0.16'>
       <g stroke='${c}' stroke-width='2'>
         <rect x='15' y='35' width='60' height='35' rx='2'/>
         <path d='M15 35 l60 -8 l-2 8'/>
         <line x1='25' y1='30' x2='27' y2='35'/>
         <line x1='40' y1='27' x2='42' y2='32'/>
         <line x1='55' y1='25' x2='57' y2='30'/>
       </g>
     </svg>`,
  );
}

function confettiSvg(): string {
  // Multi-color celebration confetti — uses 4 hues for the festive feel.
  return svgUrl(
    `<svg xmlns='http://www.w3.org/2000/svg' width='90' height='90' viewBox='0 0 90 90' opacity='0.22'>
       <g stroke-width='2' fill='none'>
         <line x1='15' y1='15' x2='22' y2='8' stroke='#d93025'/>
         <line x1='75' y1='15' x2='68' y2='8' stroke='#1a73e8'/>
         <line x1='15' y1='75' x2='22' y2='82' stroke='#34a853'/>
         <line x1='75' y1='75' x2='68' y2='82' stroke='#fbbc04'/>
         <circle cx='45' cy='20' r='3' fill='#d93025'/>
         <circle cx='25' cy='45' r='3' fill='#1a73e8'/>
         <circle cx='65' cy='45' r='3' fill='#34a853'/>
         <circle cx='45' cy='70' r='3' fill='#fbbc04'/>
       </g>
     </svg>`,
  );
}

/** Coerce arbitrary input to a valid pattern key (defaults to `""`).
 *  Rust validates server-side too, but defensively normalising on the
 *  read path means stale fixtures or hand-edited DBs never blow up. */
export function normalizePattern(raw: string | null | undefined): BackgroundPatternKey {
  if (!raw) return "";
  if (raw in BACKGROUND_PATTERNS) return raw as BackgroundPatternKey;
  return "";
}
