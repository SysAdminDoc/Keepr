import { useState } from "react";
import { Pin, Tag, ChevronDown, Image as ImageIcon, Bell, Lock, Sparkles } from "lucide-react";
import clsx from "clsx";
import { useStore } from "../store";
import { api } from "../api";
import { COLOR_KEYS, COLOR_LABELS, LIGHT_HEX } from "../colors";
import type { ColorKey, NoteKind, SearchFilters } from "../types";

const KIND_LABEL: Record<NoteKind, string> = {
  text: "Notes",
  list: "Lists",
};

/**
 * Compact chip row for NF-09 — sits under the top bar. Each facet group
 * exposes a popover; activating chips ORs within a group; groups AND
 * across each other. "Clear filters" appears when anything is active.
 */
export function FilterChips() {
  const filters = useStore((s) => s.filters);
  const setFilters = useStore((s) => s.setFilters);
  const clearFilters = useStore((s) => s.clearFilters);
  const labels = useStore((s) => s.labels);
  const section = useStore((s) => s.section);
  const vaultInitialized = useStore((s) => s.vaultInitialized);
  const vaultUnlocked = useStore((s) => s.vaultUnlocked);
  // EI-V0.5-5 — Pinned filter is always empty in Trash (set_trashed
  // clears pinned). Hide the chip there to avoid a "click does nothing"
  // experience.
  const showPinnedChip = section.kind !== "trash";
  // Vault chip only meaningful when the vault is both initialized AND
  // unlocked — locked-vault notes look empty so a filter for them
  // returns nothing useful.
  const showVaultChip = vaultInitialized && vaultUnlocked;

  const totalFacets =
    filters.kinds.length +
    filters.colors.length +
    filters.labelIds.length +
    (filters.pinnedOnly ? 1 : 0) +
    (filters.hasImage ? 1 : 0) +
    (filters.hasReminder ? 1 : 0) +
    (filters.inVault ? 1 : 0);

  // Don't render if nothing to show and no menus to open — defer to the
  // small "Filters" entry in the search bar's adjacent slot.
  const hasAny = totalFacets > 0;

  const toggleKind = (k: NoteKind) => {
    const next: SearchFilters = {
      ...filters,
      kinds: filters.kinds.includes(k)
        ? filters.kinds.filter((x) => x !== k)
        : [...filters.kinds, k],
    };
    setFilters(next);
  };
  const toggleColor = (c: ColorKey) => {
    const next: SearchFilters = {
      ...filters,
      colors: filters.colors.includes(c)
        ? filters.colors.filter((x) => x !== c)
        : [...filters.colors, c],
    };
    setFilters(next);
  };
  const toggleLabel = (id: string) => {
    const next: SearchFilters = {
      ...filters,
      labelIds: filters.labelIds.includes(id)
        ? filters.labelIds.filter((x) => x !== id)
        : [...filters.labelIds, id],
    };
    setFilters(next);
  };
  const togglePinned = () =>
    setFilters({ ...filters, pinnedOnly: !filters.pinnedOnly });
  const toggleHasImage = () =>
    setFilters({ ...filters, hasImage: !filters.hasImage });
  const toggleHasReminder = () =>
    setFilters({ ...filters, hasReminder: !filters.hasReminder });
  const toggleInVault = () =>
    setFilters({ ...filters, inVault: !filters.inVault });

  return (
    <div className="flex flex-wrap items-center gap-2 max-w-5xl mx-auto px-4 sm:px-8 pt-2 pb-1">
      <KindMenu activeKinds={filters.kinds} onToggle={toggleKind} />
      <ColorMenu activeColors={filters.colors} onToggle={toggleColor} />
      {labels.length > 0 && (
        <LabelMenu
          allLabels={labels}
          activeIds={filters.labelIds}
          onToggle={toggleLabel}
        />
      )}
      {showPinnedChip && (
        <button
          type="button"
          onClick={togglePinned}
          aria-pressed={filters.pinnedOnly}
          className={clsx(
            chipBase,
            filters.pinnedOnly ? chipActive : chipInactive,
          )}
        >
          <Pin size={14} aria-hidden /> Pinned
        </button>
      )}
      <button
        type="button"
        onClick={toggleHasImage}
        aria-pressed={filters.hasImage}
        className={clsx(chipBase, filters.hasImage ? chipActive : chipInactive)}
      >
        <ImageIcon size={14} aria-hidden /> Has image
      </button>
      <button
        type="button"
        onClick={toggleHasReminder}
        aria-pressed={filters.hasReminder}
        className={clsx(chipBase, filters.hasReminder ? chipActive : chipInactive)}
      >
        <Bell size={14} aria-hidden /> Has reminder
      </button>
      {showVaultChip && (
        <button
          type="button"
          onClick={toggleInVault}
          aria-pressed={filters.inVault}
          className={clsx(chipBase, filters.inVault ? chipActive : chipInactive)}
        >
          <Lock size={14} aria-hidden /> In vault
        </button>
      )}
      {hasAny && (
        <button
          type="button"
          onClick={clearFilters}
          className="ml-1 text-xs text-[var(--keepr-accent)] hover:underline"
        >
          Clear filters
        </button>
      )}
      {hasAny && <SaveAsSmartLabelButton filters={filters} />}
    </div>
  );
}

/** v0.22.2 — appears next to "Clear filters" when any facet is active.
 *  Prompts for a name and persists the current filters as a sidebar
 *  Smart Label that re-applies them on click. */
function SaveAsSmartLabelButton({ filters }: { filters: SearchFilters }) {
  const showToast = useStore((s) => s.showToast);
  const load = useStore((s) => s.load);
  const onClick = async () => {
    const name = window.prompt("Name this Smart Label:")?.trim();
    if (!name) return;
    try {
      await api.createSmartLabel(name, JSON.stringify(filters));
      await load();
      showToast(`Saved as Smart Label "${name}"`);
    } catch (e) {
      showToast("Could not save: " + String(e));
    }
  };
  return (
    <button
      type="button"
      onClick={onClick}
      className="ml-1 inline-flex items-center gap-1 text-xs text-[var(--keepr-accent)] hover:underline"
    >
      <Sparkles size={12} aria-hidden /> Save as Smart Label
    </button>
  );
}

const chipBase =
  "inline-flex items-center gap-1 px-3 py-1 text-xs rounded border transition-colors";
const chipActive =
  "bg-[#feefc3] dark:bg-[#41331c] border-[#fbbc04] text-[#202124] dark:text-[#fdd663]";
const chipInactive =
  "border-gray-300 dark:border-[#5f6368] text-gray-700 dark:text-gray-300 hover:bg-black/5 dark:hover:bg-white/10";

function KindMenu({
  activeKinds,
  onToggle,
}: {
  activeKinds: NoteKind[];
  onToggle: (k: NoteKind) => void;
}) {
  const [open, setOpen] = useState(false);
  const summary =
    activeKinds.length === 0
      ? "Type"
      : activeKinds.map((k) => KIND_LABEL[k]).join(", ");
  return (
    <div className="relative">
      <button
        type="button"
        aria-haspopup="true"
        aria-expanded={open}
        onClick={() => setOpen((v) => !v)}
        className={clsx(
          chipBase,
          activeKinds.length > 0 ? chipActive : chipInactive,
        )}
      >
        {summary} <ChevronDown size={12} aria-hidden />
      </button>
      {open && (
        <div
          className="absolute z-30 top-9 left-0 w-40 rounded-lg shadow-lg border bg-white dark:bg-[#2d2e30] dark:border-[#5f6368] p-2"
          onClick={(e) => e.stopPropagation()}
        >
          {(["text", "list"] as NoteKind[]).map((k) => (
            <label
              key={k}
              className="flex items-center gap-2 px-2 py-1 rounded hover:bg-black/5 dark:hover:bg-white/10 cursor-pointer text-sm"
            >
              <input
                type="checkbox"
                checked={activeKinds.includes(k)}
                onChange={() => onToggle(k)}
              />
              {KIND_LABEL[k]}
            </label>
          ))}
          <button
            type="button"
            onClick={() => setOpen(false)}
            className="mt-1 w-full text-xs text-right text-[var(--keepr-accent)] hover:underline px-2"
          >
            Done
          </button>
        </div>
      )}
    </div>
  );
}

function ColorMenu({
  activeColors,
  onToggle,
}: {
  activeColors: ColorKey[];
  onToggle: (c: ColorKey) => void;
}) {
  const [open, setOpen] = useState(false);
  return (
    <div className="relative">
      <button
        type="button"
        aria-haspopup="true"
        aria-expanded={open}
        onClick={() => setOpen((v) => !v)}
        className={clsx(
          chipBase,
          activeColors.length > 0 ? chipActive : chipInactive,
        )}
      >
        {activeColors.length === 0
          ? "Color"
          : `Color (${activeColors.length})`}{" "}
        <ChevronDown size={12} aria-hidden />
      </button>
      {open && (
        <div
          className="absolute z-30 top-9 left-0 grid grid-cols-6 gap-1 p-2 bg-white dark:bg-[#2d2e30] rounded-lg shadow-lg border border-gray-200 dark:border-[#5f6368]"
          onClick={(e) => e.stopPropagation()}
        >
          {COLOR_KEYS.map((k) => {
            const selected = activeColors.includes(k);
            return (
              <button
                key={k}
                type="button"
                onClick={() => onToggle(k)}
                aria-pressed={selected}
                title={COLOR_LABELS[k]}
                className={clsx(
                  "w-7 h-7 rounded-full border transition-transform hover:scale-110 motion-reduce:transform-none",
                  selected
                    ? "ring-2 ring-[var(--keepr-accent)] border-transparent"
                    : k === "default"
                    ? "border-gray-400"
                    : "border-transparent",
                )}
                style={{
                  background: k === "default" ? "transparent" : LIGHT_HEX[k],
                }}
              />
            );
          })}
        </div>
      )}
    </div>
  );
}

function LabelMenu({
  allLabels,
  activeIds,
  onToggle,
}: {
  allLabels: { id: string; name: string }[];
  activeIds: string[];
  onToggle: (id: string) => void;
}) {
  const [open, setOpen] = useState(false);
  const summary =
    activeIds.length === 0
      ? "Label"
      : activeIds.length === 1
      ? allLabels.find((l) => l.id === activeIds[0])?.name ?? "Label"
      : `Label (${activeIds.length})`;
  return (
    <div className="relative">
      <button
        type="button"
        aria-haspopup="true"
        aria-expanded={open}
        onClick={() => setOpen((v) => !v)}
        className={clsx(
          chipBase,
          activeIds.length > 0 ? chipActive : chipInactive,
        )}
      >
        <Tag size={14} aria-hidden /> {summary} <ChevronDown size={12} aria-hidden />
      </button>
      {open && (
        <div
          className="absolute z-30 top-9 left-0 w-56 rounded-lg shadow-lg border bg-white dark:bg-[#2d2e30] dark:border-[#5f6368] p-2"
          onClick={(e) => e.stopPropagation()}
        >
          <div className="max-h-48 overflow-y-auto">
            {allLabels.map((l) => (
              <label
                key={l.id}
                className="flex items-center gap-2 px-2 py-1 rounded hover:bg-black/5 dark:hover:bg-white/10 cursor-pointer text-sm"
              >
                <input
                  type="checkbox"
                  checked={activeIds.includes(l.id)}
                  onChange={() => onToggle(l.id)}
                />
                <span className="truncate">{l.name}</span>
              </label>
            ))}
          </div>
          <button
            type="button"
            onClick={() => setOpen(false)}
            className="mt-1 w-full text-xs text-right text-[var(--keepr-accent)] hover:underline px-2"
          >
            Done
          </button>
        </div>
      )}
    </div>
  );
}

