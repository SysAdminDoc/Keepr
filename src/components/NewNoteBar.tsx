import { lazy, Suspense, useState } from "react";
import { CheckSquare, Image, Paintbrush } from "lucide-react";
import { useStore } from "../store";
import { api } from "../api";

// EI-V0.5-17 — the canvas drag-in is a big chunk (event handlers,
// repaint loop, palette UI); lazy-load until the user clicks Paintbrush.
const DrawingCanvasModal = lazy(() =>
  import("./DrawingCanvasModal").then((m) => ({ default: m.DrawingCanvasModal })),
);

export function NewNoteBar() {
  const openEditor = useStore((s) => s.openEditor);
  const upsertNote = useStore((s) => s.upsertNote);
  const showToast = useStore((s) => s.showToast);
  const [drawingOpen, setDrawingOpen] = useState(false);

  const saveDrawing = async (bytes: number[]) => {
    try {
      // Create a blank note first so we have an id the attachment
      // command can attach to.
      const note = await api.createNote({
        kind: "text",
        title: "",
        body: "",
        color: "default",
        pinned: false,
        checklist: [],
        labels: [],
        backgroundPattern: "",
      });
      const att = await api.addImageAttachmentBytes(
        note.id,
        bytes,
        "image/png",
        "drawing.png",
      );
      const withAtt = { ...note, attachments: [att] };
      upsertNote(withAtt);
      setDrawingOpen(false);
      openEditor(note.id);
      showToast("Drawing saved");
    } catch (e) {
      showToast("Could not save drawing: " + String(e));
    }
  };

  return (
    <div className="w-full max-w-xl mx-auto mb-8">
      <div className="rounded-lg border border-gray-200 dark:border-[#5f6368] shadow-keep bg-white dark:bg-[#202124] flex items-center px-4 py-3">
        <button
          type="button"
          className="flex-1 text-left text-base text-gray-600 dark:text-gray-300 font-medium outline-none"
          onClick={() => openEditor(null)}
          aria-label="Take a note"
        >
          Take a note…
        </button>
        <div className="flex items-center gap-1 text-gray-600 dark:text-gray-300">
          <button
            type="button"
            onClick={() => openEditor(null)}
            aria-label="New list"
            title="New list"
            className="p-2 rounded-full hover:bg-gray-100 dark:hover:bg-[#3c4043]"
          >
            <CheckSquare size={20} aria-hidden />
          </button>
          <button
            type="button"
            onClick={() => openEditor(null)}
            aria-label="New note with image (open editor first)"
            title="Open editor to add an image"
            className="p-2 rounded-full hover:bg-gray-100 dark:hover:bg-[#3c4043]"
          >
            <Image size={20} aria-hidden />
          </button>
          <button
            type="button"
            onClick={() => setDrawingOpen(true)}
            aria-label="New drawing"
            title="New drawing"
            className="p-2 rounded-full hover:bg-gray-100 dark:hover:bg-[#3c4043]"
          >
            <Paintbrush size={20} aria-hidden />
          </button>
        </div>
      </div>
      {drawingOpen && (
        <Suspense fallback={null}>
          <DrawingCanvasModal
            open={drawingOpen}
            onCancel={() => setDrawingOpen(false)}
            onSave={saveDrawing}
          />
        </Suspense>
      )}
    </div>
  );
}
