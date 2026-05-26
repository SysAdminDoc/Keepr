import { CheckSquare, Image, Paintbrush } from "lucide-react";
import { useStore } from "../store";

export function NewNoteBar() {
  const openEditor = useStore((s) => s.openEditor);
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
            disabled
            aria-label="Add drawing (coming in v0.5)"
            title="Drawing (coming v0.5)"
            aria-disabled="true"
            className="p-2 rounded-full opacity-40 cursor-not-allowed"
          >
            <Paintbrush size={20} aria-hidden />
          </button>
        </div>
      </div>
    </div>
  );
}
