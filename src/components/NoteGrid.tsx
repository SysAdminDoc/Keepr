import Masonry from "react-masonry-css";
import type { Note } from "../types";
import { NoteCard } from "./NoteCard";

interface Props {
  notes: Note[];
}

const breakpoints = {
  default: 5,
  1600: 5,
  1400: 4,
  1100: 3,
  800: 2,
  500: 1,
};

export function NoteGrid({ notes }: Props) {
  return (
    <Masonry
      breakpointCols={breakpoints}
      className="masonry-grid"
      columnClassName="masonry-grid-col"
    >
      {notes.map((n) => (
        <NoteCard key={n.id} note={n} />
      ))}
    </Masonry>
  );
}
