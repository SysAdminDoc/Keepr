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
import { sortNotes, useStore } from "../store";
import { api } from "../api";
import { useState } from "react";

interface Props {
  notes: Note[];
  /** "masonry" (default) is the CSS multi-column layout used for the
   *  unpinned notes — cards pack tightly top-to-bottom across columns.
   *
   *  "stable-grid" is a row-major CSS Grid used for the pinned section,
   *  where each card is placed at its `position` slot and any "gaps"
   *  (caused by unpinning a middle card) render as invisible
   *  placeholders. This keeps the OTHER pinned cards in their exact
   *  visual cell so the user's muscle memory holds — Keep's masonry
   *  reflow on unpin was the original gripe. */
  layout?: "masonry" | "stable-grid";
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
export function NoteGrid({ notes, layout = "masonry" }: Props) {
  const viewMode = useStore((s) => s.viewMode);
  const sortMode = useStore((s) => s.sortMode);
  const setSortMode = useStore((s) => s.setSortMode);
  const section = useStore((s) => s.section);
  const showToast = useStore((s) => s.showToast);
  const cardWidth = useStore((s) => s.cardWidth);

  // EI-V0.5-1 — drag-reorder is only safe in the Notes section. In
  // Archive/Trash/Label sections, a drop would write `position` for the
  // dragged ids only, leaving every other note with stale positions and
  // corrupting Custom-sort ordering on the next full load. Drag works
  // in every sort mode now; on a drop under non-Custom modes we flip
  // sortMode to "custom" below so the visible order matches what the
  // user just dragged into place.
  const dragEnabled = section.kind === "notes";

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

      // Optimistic: patch position on every visible note + immediately
      // re-sort the notes array so the dropped card actually lands at
      // the drop site. Without the re-sort, the array order in the
      // store is whatever the previous sort left behind — the position
      // FIELD updates but the rendered ORDER doesn't, so the drag
      // appears to snap back.
      //
      // For unpinned drags under non-Custom modes, the user's intent
      // only becomes visible once sortMode flips to Custom (under
      // Modified the unpinned subset re-sorts by updated_at again).
      // Compute the target sort up front and apply both the patch and
      // the re-sort in a single setState so the render is atomic — no
      // visible snap-back between drop and "Switched to Custom" toast.
      const positionedIds = reordered.map((n) => n.id);
      const draggedAllPinned = notes.every((n) => n.pinned);
      const targetSort =
        draggedAllPinned || sortMode === "custom" ? sortMode : "custom";
      useStore.setState((s) => {
        const next = s.notes.map((n) => {
          const i = positionedIds.indexOf(n.id);
          return i >= 0 ? { ...n, position: i } : n;
        });
        return { notes: sortNotes(next, targetSort) };
      });
      // Persist the sortMode change (if any) so it survives reload —
      // setSortMode also runs sortNotes internally, but our setState
      // above already produced the correct order so the extra re-sort
      // is a no-op visually.
      if (targetSort !== sortMode) {
        setSortMode(targetSort);
        showToast("Switched to Custom sort to keep your order");
      }
      // Persist positions. The Rust command updates position for every
      // id passed in. We pass the full visible set (the only notes
      // whose order changed).
      try {
        await api.reorderNotes(positionedIds);
      } catch (err) {
        showToast("Could not reorder: " + String(err));
      }
    },
    // We deliberately don't depend on allNotes — its identity changes on
    // every store update and would re-create this callback unnecessarily.
    [notes, showToast, sortMode, setSortMode],
  );

  // EI-10 — replaced react-masonry-css (last release Aug 2022, no
  // virtualization, blocking NF-05 work) with CSS multi-column layout.
  // `column-width` (instead of `column-count`) + `break-inside: avoid`
  // lets the browser fit as many columns as the container can hold at
  // the user's preferred card width — so Ctrl+Wheel "zoom" just bumps
  // `cardWidth` and the layout reflows itself, no JS needed for the
  // breakpoint math. List mode collapses to one column.
  //
  // EI-V0.5-NEXT — "stable-grid" branch (pinned section only) renders a
  // row-major CSS Grid where each card sits in the cell its `position`
  // points to and any missing positions render as invisible
  // placeholders. Unpinning a middle card leaves a visible gap so the
  // remaining cards never visually move. The multi-column masonry above
  // would redistribute columns on every count change, scrambling the
  // user's muscle memory for pinned-note locations.
  let cards: JSX.Element;
  if (viewMode === "list") {
    cards = (
      <div className="max-w-3xl mx-auto">
        {notes.map((n) => (
          <div key={n.id} className="mb-4">
            <NoteCard note={n} />
          </div>
        ))}
      </div>
    );
  } else if (layout === "stable-grid") {
    // Build a sparse slot array from positions. We deliberately preserve
    // gaps (positions that no current note occupies) so the visible
    // grid cells of the remaining cards don't shift after an unpin.
    const byPos = [...notes].sort((a, b) => a.position - b.position);
    const maxPos = byPos.length > 0 ? byPos[byPos.length - 1].position : -1;
    const slots: (Note | null)[] = new Array(maxPos + 1).fill(null);
    for (const n of byPos) slots[n.position] = n;
    cards = (
      <div
        style={{
          display: "grid",
          gridTemplateColumns: `repeat(auto-fill, minmax(${cardWidth}px, 1fr))`,
          gap: "1rem",
          alignItems: "start",
        }}
      >
        {slots.map((n, i) =>
          n ? (
            <NoteCard key={n.id} note={n} />
          ) : (
            // Invisible placeholder — keeps the grid cell occupied so
            // surrounding cards stay put. aria-hidden so screen readers
            // skip it; visibility:hidden so the cell still claims space
            // in row-height calculations. v0.21.2 — `min-height: 1px`
            // guards against the all-placeholders-in-a-row case where
            // CSS Grid would otherwise collapse the row to zero and
            // cards below would jump up. 1px is enough to keep the
            // row claimed without producing a visible gap-row band.
            <div
              key={`gap-${i}`}
              aria-hidden
              style={{ visibility: "hidden", minHeight: "1px" }}
            />
          ),
        )}
      </div>
    );
  } else {
    cards = (
      <div className="gap-4" style={{ columnWidth: `${cardWidth}px`, columnGap: "1rem" }}>
        {notes.map((n) => (
          // `break-inside-avoid` keeps a card from being split across
          // columns; the inline-block + w-full pairing is the Tailwind
          // recipe for masonry-with-multicol that works in every modern
          // browser back to Chrome 50 / Firefox 52 / Safari 9.
          <div key={n.id} className="break-inside-avoid mb-4 inline-block w-full">
            <NoteCard note={n} />
          </div>
        ))}
      </div>
    );
  }

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
