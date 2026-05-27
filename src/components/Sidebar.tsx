import { useMemo } from "react";
import { Lightbulb, Tag, Archive, Trash2, Pencil, Bell, Sparkles, X } from "lucide-react";
import { isActive as isReminderActive } from "../lib/reminders";
import clsx from "clsx";
import { useStore } from "../store";
import { api } from "../api";
import type { Section, SearchFilters } from "../types";
import { EMPTY_FILTERS } from "../types";

interface Props {
  expanded: boolean;
}

const navItem =
  "flex items-center h-12 pr-6 rounded-r-full select-none transition-colors text-left";

export function Sidebar({ expanded }: Props) {
  const section = useStore((s) => s.section);
  const setSection = useStore((s) => s.setSection);
  const setFilters = useStore((s) => s.setFilters);
  const labels = useStore((s) => s.labels);
  const notes = useStore((s) => s.notes);
  const reminders = useStore((s) => s.reminders);
  const smartLabels = useStore((s) => s.smartLabels);
  const showToast = useStore((s) => s.showToast);
  const load = useStore((s) => s.load);
  const openLabelsManager = useStore((s) => s.openLabelsManager);

  // NF-V0.5-A — badge count of active (non-fired, non-dismissed) reminders.
  // Recurring reminders never set `firedAt`, so they stay counted between
  // fires and the badge always reflects what's actually pending.
  const reminderCount = useMemo(
    () => reminders.filter(isReminderActive).length,
    [reminders],
  );

  // NF-V0.5-H — per-label note counts. Computed once per labels/notes
  // change, not per render. Excludes trashed notes (they're not visible
  // when clicking a label anyway).
  const labelCounts = useMemo(() => {
    const counts = new Map<string, number>();
    for (const n of notes) {
      if (n.trashed) continue;
      for (const id of n.labels) {
        counts.set(id, (counts.get(id) ?? 0) + 1);
      }
    }
    return counts;
  }, [notes]);

  const isActive = (s: Section): boolean => {
    if (section.kind === "label" && s.kind === "label")
      return section.labelId === s.labelId;
    if (section.kind === "smart" && s.kind === "smart")
      return section.smartLabelId === s.smartLabelId;
    return section.kind === s.kind;
  };

  const activateSmart = (id: string, queryJson: string) => {
    setSection({ kind: "smart", smartLabelId: id });
    try {
      const parsed = JSON.parse(queryJson) as Partial<SearchFilters>;
      setFilters({ ...EMPTY_FILTERS, ...parsed });
    } catch {
      // Bad payload: clear filters but still navigate so the user can
      // see the empty results and decide to delete the smart label.
      setFilters(EMPTY_FILTERS);
      showToast("Smart Label payload was unreadable — filters cleared.");
    }
  };

  const removeSmart = async (id: string, name: string) => {
    if (!confirm(`Delete Smart Label "${name}"?`)) return;
    try {
      await api.deleteSmartLabel(id);
      if (section.kind === "smart" && section.smartLabelId === id) {
        setSection({ kind: "notes" });
        setFilters(EMPTY_FILTERS);
      }
      await load();
      showToast(`Deleted "${name}"`);
    } catch (e) {
      showToast("Delete failed: " + String(e));
    }
  };

  const item = (
    s: Section,
    icon: React.ReactNode,
    label: string,
    count?: number,
  ) => {
    const active = isActive(s);
    return (
      <li key={s.kind === "label" ? `label:${s.labelId}` : s.kind}>
        <button
          type="button"
          className={clsx(
            navItem,
            "w-full",
            active
              ? "bg-[#feefc3] dark:bg-[#41331c] text-[#202124] dark:text-[#fdd663] font-medium"
              : "hover:bg-gray-200 dark:hover:bg-[#3c4043]",
            expanded ? "pl-6" : "pl-3 justify-center pr-3",
          )}
          onClick={() => setSection(s)}
          aria-current={active ? "page" : undefined}
          aria-label={count !== undefined ? `${label}, ${count} notes` : label}
          title={label}
        >
          <span className="w-6 grid place-items-center" aria-hidden>
            {icon}
          </span>
          {expanded && (
            <span className="ml-7 truncate flex-1">{label}</span>
          )}
          {expanded && count !== undefined && count > 0 && (
            <span
              className={clsx(
                "ml-2 text-xs tabular-nums",
                active ? "opacity-90" : "opacity-50",
              )}
              aria-hidden
            >
              {count}
            </span>
          )}
        </button>
      </li>
    );
  };

  return (
    <aside
      className={clsx(
        "shrink-0 overflow-y-auto overflow-x-hidden bg-white dark:bg-[#202124] border-r border-gray-200 dark:border-[#5f6368] transition-[width] duration-150 motion-reduce:transition-none",
        expanded ? "w-72" : "w-14",
      )}
      aria-label="Sections"
    >
      <ul className="py-2">
        {item({ kind: "notes" }, <Lightbulb size={22} />, "Notes")}
        {item(
          { kind: "reminders" },
          <Bell size={20} />,
          "Reminders",
          reminderCount,
        )}
        {expanded && (
          <li className="px-6 pt-4 pb-1 text-xs font-medium text-gray-500 uppercase tracking-wide">
            Labels
          </li>
        )}
        {labels.map((l) =>
          item(
            { kind: "label", labelId: l.id },
            <Tag size={20} />,
            l.name,
            labelCounts.get(l.id) ?? 0,
          ),
        )}
        <li>
          <button
            type="button"
            className={clsx(
              navItem,
              "w-full hover:bg-gray-200 dark:hover:bg-[#3c4043]",
              expanded ? "pl-6" : "pl-3 justify-center pr-3",
            )}
            onClick={openLabelsManager}
            aria-label="Edit labels"
            title="Edit labels"
          >
            <span className="w-6 grid place-items-center" aria-hidden>
              <Pencil size={20} />
            </span>
            {expanded && <span className="ml-7">Edit labels</span>}
          </button>
        </li>
        {item({ kind: "archive" }, <Archive size={20} />, "Archive")}
        {item({ kind: "trash" }, <Trash2 size={20} />, "Trash")}
        {smartLabels.length > 0 && expanded && (
          <li className="px-6 pt-4 pb-1 text-xs font-medium text-gray-500 uppercase tracking-wide">
            Smart Labels
          </li>
        )}
        {smartLabels.map((s) => {
          const active = isActive({ kind: "smart", smartLabelId: s.id });
          return (
            <li key={`smart:${s.id}`}>
              <button
                type="button"
                onClick={() => activateSmart(s.id, s.queryJson)}
                onContextMenu={(e) => {
                  e.preventDefault();
                  void removeSmart(s.id, s.name);
                }}
                className={clsx(
                  navItem,
                  "w-full group",
                  active
                    ? "bg-[#feefc3] dark:bg-[#41331c] text-[#202124] dark:text-[#fdd663] font-medium"
                    : "hover:bg-gray-200 dark:hover:bg-[#3c4043]",
                  expanded ? "pl-6" : "pl-3 justify-center pr-3",
                )}
                aria-current={active ? "page" : undefined}
                aria-label={`Smart Label: ${s.name} (right-click to delete)`}
                title={`${s.name} — right-click to delete`}
              >
                <span className="w-6 grid place-items-center" aria-hidden>
                  <Sparkles size={20} />
                </span>
                {expanded && <span className="ml-7 truncate flex-1">{s.name}</span>}
                {expanded && (
                  <span
                    role="button"
                    tabIndex={-1}
                    onClick={(e) => {
                      e.stopPropagation();
                      void removeSmart(s.id, s.name);
                    }}
                    aria-label={`Delete ${s.name}`}
                    className="opacity-0 group-hover:opacity-60 hover:opacity-100 p-1 rounded"
                  >
                    <X size={14} aria-hidden />
                  </span>
                )}
              </button>
            </li>
          );
        })}
      </ul>
    </aside>
  );
}
