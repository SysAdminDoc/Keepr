import { Lightbulb, Tag, Archive, Trash2, Pencil } from "lucide-react";
import clsx from "clsx";
import { useStore } from "../store";
import type { Section } from "../types";

interface Props {
  expanded: boolean;
}

const navItem =
  "flex items-center h-12 pr-6 rounded-r-full cursor-pointer select-none transition-colors";

export function Sidebar({ expanded }: Props) {
  const { section, setSection, labels, openLabelsManager } = useStore();

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
      <li
        className={clsx(
          navItem,
          active
            ? "bg-[#feefc3] dark:bg-[#41331c] text-[#202124] dark:text-[#fdd663] font-medium"
            : "hover:bg-gray-200 dark:hover:bg-[#3c4043]",
          expanded ? "pl-6" : "pl-3 justify-center pr-3",
        )}
        onClick={() => setSection(s)}
        title={label}
      >
        <span className="w-6 grid place-items-center">{icon}</span>
        {expanded && <span className="ml-7 truncate">{label}</span>}
      </li>
    );
  };

  return (
    <aside
      className={clsx(
        "shrink-0 overflow-y-auto overflow-x-hidden bg-white dark:bg-[#202124] border-r border-gray-200 dark:border-[#5f6368] transition-[width] duration-150",
        expanded ? "w-72" : "w-14",
      )}
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
        <li
          className={clsx(
            navItem,
            "hover:bg-gray-200 dark:hover:bg-[#3c4043]",
            expanded ? "pl-6" : "pl-3 justify-center pr-3",
          )}
          onClick={openLabelsManager}
          title="Edit labels"
        >
          <span className="w-6 grid place-items-center">
            <Pencil size={20} />
          </span>
          {expanded && <span className="ml-7">Edit labels</span>}
        </li>
        {item({ kind: "archive" }, <Archive size={20} />, "Archive")}
        {item({ kind: "trash" }, <Trash2 size={20} />, "Trash")}
      </ul>
    </aside>
  );
}
