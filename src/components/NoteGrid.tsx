import { useCallback } from "react";
import {
  DndContext,
  DragOverlay,
  PointerSensor,
  closestCenter,
  useSensor,
  useSensors,
  type DragEndEvent,
  type DragStartEvent,
} from "@dnd-kit/core";
import { SortableContext, arrayMove, rectSortingStrategy } from "@dnd-kit/sortable";
import type { Note } from "../types";
import { NoteCard } from "./NoteCard";
import { useStore } from "../store";
import { api } from "../api";
import { useState } from "react";

interface Props {
  notes: Note[];
}

/**
 * Renders the masonry grid. When sortMode === "custom" we additionally
 * wrap children in DndContext + SortableContext (NF-05) so a drag inside
 * this grid reorders the notes in-place and persists via api.reorderNotes.
 *
 * Drag-reorder is intentionally disabled in non-custom sort modes — the
 * order is computed from updated_at / created_at / title there and a drop
 * would feel like it had no effect. Users have to switch to Custom in the
 * Sort menu to get drag handles.
 */
export function NoteGrid({ notes }: Props) {
  const viewMode = useStore((s) => s.viewMode);
  const sortMode = useStore((s) => s.sortMode);
  const section = useStore((s) => s.section);
  const showToast = useStore((s) => s.showToast);

  // EI-V0.5-1 — drag-reorder is only safe in the Notes section. In
  // Archive/Trash/Label sections, a drop would write `position` for the
  // dragged ids only, leaving every other note with stale positions and
  // corrupting Custom-sort ordering on the next full load.
  const dragEnabled = sortMode === "custom" && section.kind === "notes";

  const sensors = useSensors(
    // Require a small pointer drag distance before starting — so click-to-
    // open-editor still works without a drag intent.
    useSensor(PointerSensor, { activationConstraint: { distance: 6 } }),
  );

  const [activeId, setActiveId] = useState<string | null>(null);
  const activeNote = activeId ? notes.find((n) => n.id === activeId) ?? null : null;

  const onDragStart = useCallback((e: DragStartEvent) => {
    setActiveId(String(e.active.id));
  }, []);

  const onDragEnd = useCallback(
    async (e: DragEndEvent) => {
      setActiveId(null);
      const { active, over } = e;
      if (!over || active.id === over.id) return;
      const ids = notes.map((n) => n.id);
      const oldIndex = ids.indexOf(String(active.id));
      const newIndex = ids.indexOf(String(over.id));
      if (oldIndex === -1 || newIndex === -1) return;
      const reordered = arrayMove(notes, oldIndex, newIndex);

      // Optimistic: patch position on every visible note. Notes outside
      // this section keep their positions so cross-section ordering is
      // stable.
      const positionedIds = reordered.map((n) => n.id);
      useStore.setState((s) => {
        const next = s.notes.map((n) => {
          const i = positionedIds.indexOf(n.id);
          return i >= 0 ? { ...n, position: i } : n;
        });
        return { notes: next };
      });
      // Persist. The Rust command updates position for every id passed in.
      // We pass the full visible set (the only notes whose order changed).
      try {
        await api.reorderNotes(positionedIds);
      } catch (err) {
        showToast("Could not reorder: " + String(err));
      }
    },
    // We deliberately don't depend on allNotes — its identity changes on
    // every store update and would re-create this callback unnecessarily.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [notes, showToast],
  );

  // EI-10 — replaced react-masonry-css (last release Aug 2022, no
  // virtualization, blocking NF-05 work) with CSS multi-column layout.
  // `column-count` + `break-inside: avoid` produces the same visual
  // (cards fill columns top-to-bottom) with zero runtime cost and no
  // ResizeObserver shim needed. List mode collapses to one column.
  const cards = (
    <div
      className={
        viewMode === "list"
          ? "max-w-3xl mx-auto"
          : "columns-1 sm:columns-2 lg:columns-3 xl:columns-4 2xl:columns-5 gap-4"
      }
    >
      {notes.map((n) => (
        // `break-inside-avoid` keeps a card from being split across
        // columns; the inline-block + w-full pairing is the Tailwind
        // recipe for masonry-with-multicol that works in every modern
        // browser back to Chrome 50 / Firefox 52 / Safari 9.
        <div
          key={n.id}
          className={
            viewMode === "list" ? "mb-4" : "break-inside-avoid mb-4 inline-block w-full"
          }
        >
          <NoteCard note={n} />
        </div>
      ))}
    </div>
  );

  if (!dragEnabled) return cards;

  return (
    <DndContext
      sensors={sensors}
      collisionDetection={closestCenter}
      onDragStart={onDragStart}
      onDragEnd={onDragEnd}
    >
      <SortableContext items={notes.map((n) => n.id)} strategy={rectSortingStrategy}>
        {cards}
      </SortableContext>
      <DragOverlay>
        {activeNote ? (
          <div className="opacity-90 pointer-events-none">
            <NoteCard note={activeNote} />
          </div>
        ) : null}
      </DragOverlay>
    </DndContext>
  );
}
