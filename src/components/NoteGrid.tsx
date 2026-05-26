import Masonry from "react-masonry-css";
import type { Note } from "../types";
import { NoteCard } from "./NoteCard";
import { useStore } from "../store";

interface Props {
  notes: Note[];
}

const gridBreakpoints = {
  default: 5,
  1600: 5,
  1400: 4,
  1100: 3,
  800: 2,
  500: 1,
};

const listBreakpoints = {
  default: 1,
};

export function NoteGrid({ notes }: Props) {
  const viewMode = useStore((s) => s.viewMode);
  const breakpoints =
    viewMode === "list" ? listBreakpoints : gridBreakpoints;
  return (
    <Masonry
      breakpointCols={breakpoints}
      className={
        viewMode === "list"
          ? "masonry-grid masonry-grid-list"
          : "masonry-grid"
      }
      columnClassName="masonry-grid-col"
    >
      {notes.map((n) => (
        <NoteCard key={n.id} note={n} />
      ))}
    </Masonry>
  );
}
