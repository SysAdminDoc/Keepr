import { useRef } from "react";
import { X } from "lucide-react";
import { useEscape } from "../hooks/useEscape";
import { useFocusTrap } from "../hooks/useFocusTrap";

interface Props {
  open: boolean;
  onClose: () => void;
}

interface Row {
  keys: string[];
  description: string;
}

interface Group {
  title: string;
  rows: Row[];
}

// Canonical Keep set, see audit notes §9. Items deferred to later
// versions (multi-select x, indent ], dedent [, sub-item navigation
// n/p, drag-reorder Shift+J/K) are excluded for clarity until they
// actually do something.
const GROUPS: Group[] = [
  {
    title: "Application",
    rows: [
      { keys: ["c"], description: "Create a new note" },
      { keys: ["l"], description: "Create a new list" },
      { keys: ["/"], description: "Focus search" },
      { keys: ["Ctrl", "G"], description: "Toggle grid / list view" },
      { keys: ["?"], description: "Show this help" },
    ],
  },
  {
    title: "Navigation",
    rows: [
      { keys: ["j"], description: "Focus next note" },
      { keys: ["k"], description: "Focus previous note" },
      { keys: ["Enter"], description: "Open focused note" },
    ],
  },
  {
    title: "Focused note",
    rows: [
      { keys: ["f"], description: "Pin / unpin" },
      { keys: ["e"], description: "Archive / unarchive" },
      { keys: ["#"], description: "Move to Trash" },
    ],
  },
  {
    title: "Editor",
    rows: [
      { keys: ["Esc"], description: "Save and close" },
      { keys: ["Ctrl", "Enter"], description: "Save and close" },
    ],
  },
];

export function HelpOverlay({ open, onClose }: Props) {
  const containerRef = useRef<HTMLDivElement>(null);
  useEscape(open, onClose);
  useFocusTrap(containerRef, open);

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 z-[55] modal-backdrop grid place-items-center p-4"
      onClick={onClose}
      role="dialog"
      aria-modal="true"
      aria-labelledby="help-title"
    >
      <div
        ref={containerRef}
        className="w-full max-w-xl rounded-lg shadow-keep-hover bg-white dark:bg-[#2d2e30] text-gray-800 dark:text-gray-100"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between px-5 py-3 border-b border-gray-200 dark:border-[#5f6368]">
          <h2 id="help-title" className="text-base font-medium">
            Keyboard shortcuts
          </h2>
          <button
            onClick={onClose}
            aria-label="Close keyboard shortcuts"
            title="Close"
            className="p-2 rounded-full hover:bg-black/5 dark:hover:bg-white/10"
          >
            <X size={18} aria-hidden />
          </button>
        </div>
        <div className="px-5 py-4 grid grid-cols-1 sm:grid-cols-2 gap-x-8 gap-y-5">
          {GROUPS.map((g) => (
            <section key={g.title}>
              <h3 className="text-xs font-medium uppercase tracking-wide text-gray-500 dark:text-gray-400 mb-2">
                {g.title}
              </h3>
              <ul className="space-y-1.5">
                {g.rows.map((r) => (
                  <li key={r.description} className="flex items-center justify-between gap-3 text-sm">
                    <span className="text-gray-700 dark:text-gray-200">
                      {r.description}
                    </span>
                    <span className="flex items-center gap-1 shrink-0">
                      {r.keys.map((k, i) => (
                        <kbd
                          key={i}
                          className="px-2 py-0.5 text-xs font-mono rounded border border-gray-300 dark:border-[#5f6368] bg-gray-100 dark:bg-[#3c4043]"
                        >
                          {k}
                        </kbd>
                      ))}
                    </span>
                  </li>
                ))}
              </ul>
            </section>
          ))}
        </div>
        <div className="px-5 py-2 border-t border-gray-200 dark:border-[#5f6368] text-xs text-gray-500 dark:text-gray-400">
          Press <kbd className="px-1 font-mono rounded border border-gray-300 dark:border-[#5f6368] bg-gray-100 dark:bg-[#3c4043]">?</kbd> any time to reopen this list.
        </div>
      </div>
    </div>
  );
}
