import { Lightbulb, Tag, Archive, Trash2, Pencil } from "lucide-react";
import clsx from "clsx";
import { useStore } from "../store";
import type { Section } from "../types";

interface Props {
  expanded: boolean;
}

const navItem =
  "flex items-center h-12 pr-6 rounded-r-full select-none transition-colors text-left";

export function Sidebar({ expanded }: Props) {
  const section = useStore((s) => s.section);
  const setSection = useStore((s) => s.setSection);
  const labels = useStore((s) => s.labels);
  const openLabelsManager = useStore((s) => s.openLabelsManager);

  const isActive = (s: Section): boolean => {
    if (section.kind === "label" && s.kind === "label")
      return section.labelId === s.labelId;
    return section.kind === s.kind;
  };

  const item = (
    s: Section,
    icon: React.ReactNode,
    label: string,
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
          aria-label={label}
          title={label}
        >
          <span className="w-6 grid place-items-center" aria-hidden>
            {icon}
          </span>
          {expanded && <span className="ml-7 truncate">{label}</span>}
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
        {expanded && (
          <li className="px-6 pt-4 pb-1 text-xs font-medium text-gray-500 uppercase tracking-wide">
            Labels
          </li>
        )}
        {labels.map((l) =>
          item({ kind: "label", labelId: l.id }, <Tag size={20} />, l.name),
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
      </ul>
    </aside>
  );
}
