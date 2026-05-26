import { useState } from "react";
import { Plus, GripVertical, CheckSquare, Square, X } from "lucide-react";
import clsx from "clsx";
import {
  DndContext,
  PointerSensor,
  closestCenter,
  useSensor,
  useSensors,
  type DragEndEvent,
} from "@dnd-kit/core";
import {
  SortableContext,
  arrayMove,
  useSortable,
  verticalListSortingStrategy,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import type { ChecklistItemInput } from "../types";
import { useFlip } from "../hooks/useFlip";

/**
 * Extracted from NoteEditor.tsx in v0.16 (EI-V0.5-10) — the entire list
 * editor lives here now. Hosts: ChecklistRow (each row), the FLIP
 * animator for the move-to-bottom transition, the dnd-kit drag
 * context, and the indent/dedent helpers. Consumers pass `items` +
 * `onChange(next)`; this component owns no draft state of its own,
 * so save/discard semantics stay simple.
 */
interface Props {
  items: ChecklistItemInput[];
  onChange: (next: ChecklistItemInput[]) => void;
  /** When true, checked items sort to a collapsible group at the bottom
   *  (Keep parity). Mirrored from the global `moveCheckedToBottom` pref. */
  moveCheckedToBottom: boolean;
}

export function ChecklistSection({ items, onChange, moveCheckedToBottom }: Props) {
  const [checkedCollapsed, setCheckedCollapsed] = useState(false);

  // NF-20 polish — FLIP animator. Keyed on the checked-state bitmap so
  // the animation only runs when an item flips checked/unchecked.
  const flipKey = items.map((c) => (c.checked ? "1" : "0")).join("");
  const { register: flipRegister } = useFlip<string>(flipKey);

  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 4 } }),
  );

  const sortIdFor = (idx: number): string => {
    const it = items[idx];
    return it?.id ?? `__new:${idx}`;
  };
  const indexOfSortId = (id: string): number => {
    for (let i = 0; i < items.length; i++) {
      if (sortIdFor(i) === id) return i;
    }
    return -1;
  };

  const setItem = (idx: number, patch: Partial<ChecklistItemInput>) => {
    const next = [...items];
    next[idx] = { ...next[idx], ...patch };
    onChange(next);
  };
  const addItem = () =>
    onChange([
      ...items,
      { text: "", checked: false, position: items.length },
    ]);
  const removeItem = (idx: number) =>
    onChange(items.filter((_, i) => i !== idx));

  // NF-21 — Tab indents the row under the most recent top-level item
  // above it with a stable id; no-op when no eligible parent exists.
  const indentItem = (idx: number) => {
    if (idx <= 0) return;
    let parent: ChecklistItemInput | null = null;
    for (let i = idx - 1; i >= 0; i--) {
      const candidate = items[i];
      if (!candidate.parentId && candidate.id) {
        parent = candidate;
        break;
      }
    }
    if (!parent) return;
    const next = [...items];
    next[idx] = { ...next[idx], parentId: parent.id };
    onChange(next);
  };
  const dedentItem = (idx: number) => {
    const it = items[idx];
    if (!it.parentId) return;
    const next = [...items];
    next[idx] = { ...next[idx], parentId: null };
    onChange(next);
  };

  const onDragEnd = (e: DragEndEvent) => {
    const { active, over } = e;
    if (!over || active.id === over.id) return;
    const oldIdx = indexOfSortId(String(active.id));
    const newIdx = indexOfSortId(String(over.id));
    if (oldIdx < 0 || newIdx < 0) return;
    const reordered = arrayMove(items, oldIdx, newIdx).map((it, i) => ({
      ...it,
      position: i,
    }));
    onChange(reordered);
  };

  // Split into unchecked + checked groups; preserve original indices so
  // the row callbacks (setItem/removeItem keyed on original index) work.
  const indexed = items.map((item, originalIndex) => ({ item, originalIndex }));
  const uncheckedRows = moveCheckedToBottom
    ? indexed.filter(({ item }) => !item.checked)
    : indexed;
  const checkedRows = moveCheckedToBottom
    ? indexed.filter(({ item }) => item.checked)
    : [];
  const uncheckedSortIds = uncheckedRows.map(({ originalIndex }) =>
    sortIdFor(originalIndex),
  );

  return (
    <div className="px-2 py-1">
      <DndContext
        sensors={sensors}
        collisionDetection={closestCenter}
        onDragEnd={onDragEnd}
      >
        <SortableContext
          items={uncheckedSortIds}
          strategy={verticalListSortingStrategy}
        >
          {uncheckedRows.map(({ item: it, originalIndex: i }) => (
            <ChecklistRow
              key={sortIdFor(i)}
              sortId={sortIdFor(i)}
              draggable
              item={it}
              indented={!!it.parentId}
              onToggle={() => setItem(i, { checked: !it.checked })}
              onText={(t) => setItem(i, { text: t })}
              onEnter={addItem}
              onBackspaceEmpty={items.length > 1 ? () => removeItem(i) : undefined}
              onRemove={() => removeItem(i)}
              onIndent={() => indentItem(i)}
              onDedent={() => dedentItem(i)}
              flipRef={flipRegister(sortIdFor(i))}
            />
          ))}
        </SortableContext>
      </DndContext>
      <button
        type="button"
        onClick={addItem}
        className="flex items-center gap-2 px-3 py-2 text-sm opacity-70 hover:opacity-100"
      >
        <Plus size={18} aria-hidden /> List item
      </button>
      {checkedRows.length > 0 && (
        <div className="mt-2 pt-2 border-t border-current/10">
          <button
            type="button"
            onClick={() => setCheckedCollapsed((v) => !v)}
            aria-expanded={!checkedCollapsed}
            className="flex items-center gap-2 px-3 py-1 text-xs uppercase tracking-wide opacity-70 hover:opacity-100"
          >
            <span
              className="inline-block transition-transform motion-reduce:transition-none"
              style={{
                transform: checkedCollapsed ? "rotate(-90deg)" : "rotate(0deg)",
              }}
              aria-hidden
            >
              ▾
            </span>
            {checkedRows.length} Checked item
            {checkedRows.length === 1 ? "" : "s"}
          </button>
          {!checkedCollapsed &&
            checkedRows.map(({ item: it, originalIndex: i }) => (
              <ChecklistRow
                key={sortIdFor(i)}
                sortId={sortIdFor(i)}
                draggable={false}
                item={it}
                indented={!!it.parentId}
                onToggle={() => setItem(i, { checked: !it.checked })}
                onText={(t) => setItem(i, { text: t })}
                onEnter={addItem}
                onBackspaceEmpty={items.length > 1 ? () => removeItem(i) : undefined}
                onRemove={() => removeItem(i)}
                onIndent={() => indentItem(i)}
                onDedent={() => dedentItem(i)}
                flipRef={flipRegister(sortIdFor(i))}
              />
            ))}
        </div>
      )}
    </div>
  );
}

interface ChecklistRowProps {
  sortId: string;
  draggable: boolean;
  item: ChecklistItemInput;
  indented: boolean;
  onToggle: () => void;
  onText: (t: string) => void;
  onEnter: () => void;
  onBackspaceEmpty?: () => void;
  onRemove: () => void;
  onIndent: () => void;
  onDedent: () => void;
  flipRef?: (el: HTMLElement | null) => void;
}

function ChecklistRow({
  sortId,
  draggable,
  item,
  indented,
  onToggle,
  onText,
  onEnter,
  onBackspaceEmpty,
  onRemove,
  onIndent,
  onDedent,
  flipRef,
}: ChecklistRowProps) {
  const sortable = useSortable({ id: sortId, disabled: !draggable });
  const style: React.CSSProperties = draggable
    ? {
        transform: CSS.Transform.toString(sortable.transform),
        transition: sortable.transition,
      }
    : {};
  const mergedRef = (el: HTMLDivElement | null) => {
    sortable.setNodeRef(el);
    flipRef?.(el);
  };
  return (
    <div
      ref={mergedRef}
      style={style}
      className={clsx(
        "group/item flex items-center gap-1 px-2 py-1",
        sortable.isDragging && "opacity-50",
        indented && "pl-8",
      )}
    >
      {draggable ? (
        <button
          type="button"
          aria-label="Reorder item"
          title="Drag to reorder"
          {...sortable.attributes}
          {...sortable.listeners}
          className="p-0.5 rounded opacity-0 group-hover/item:opacity-100 focus:opacity-100 cursor-grab active:cursor-grabbing"
        >
          <GripVertical size={14} aria-hidden />
        </button>
      ) : (
        <span className="w-[18px]" aria-hidden />
      )}
      <button
        type="button"
        onClick={onToggle}
        aria-pressed={item.checked}
        aria-label={item.checked ? "Uncheck item" : "Check item"}
        className="p-1 rounded hover:bg-black/10 dark:hover:bg-white/10"
      >
        {item.checked ? (
          <CheckSquare size={18} aria-hidden />
        ) : (
          <Square size={18} aria-hidden />
        )}
      </button>
      <input
        value={item.text}
        onChange={(e) => onText(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            onEnter();
          } else if (
            e.key === "Backspace" &&
            item.text === "" &&
            onBackspaceEmpty
          ) {
            e.preventDefault();
            onBackspaceEmpty();
          } else if (e.key === "Tab") {
            e.preventDefault();
            if (e.shiftKey) onDedent();
            else onIndent();
          }
        }}
        placeholder="List item"
        aria-label="List item"
        className={clsx(
          "flex-1 bg-transparent outline-none text-[14px]",
          item.checked && "line-through opacity-60",
        )}
      />
      <button
        type="button"
        onClick={onRemove}
        aria-label="Remove item"
        className="opacity-0 group-hover/item:opacity-100 focus:opacity-100 p-1 rounded hover:bg-black/10 dark:hover:bg-white/10"
      >
        <X size={16} aria-hidden />
      </button>
    </div>
  );
}
