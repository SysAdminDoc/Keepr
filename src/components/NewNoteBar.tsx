import { CheckSquare, Image, Paintbrush } from "lucide-react";
import { useStore } from "../store";

export function NewNoteBar() {
  const { openEditor } = useStore();
  return (
    <div className="w-full max-w-xl mx-auto mb-8">
      <div className="rounded-lg border border-gray-200 dark:border-[#5f6368] shadow-keep bg-white dark:bg-[#202124] flex items-center px-4 py-3">
        <button
          className="flex-1 text-left text-base text-gray-600 dark:text-gray-300 font-medium outline-none"
          onClick={() => openEditor(null)}
        >
          Take a note…
        </button>
        <div className="flex items-center gap-1 text-gray-600 dark:text-gray-300">
          <button
            onClick={() => openEditor(null)}
            className="p-2 rounded-full hover:bg-gray-100 dark:hover:bg-[#3c4043]"
            title="New list"
          >
            <CheckSquare size={20} />
          </button>
          <button
            disabled
            title="Image (coming v0.2)"
            className="p-2 rounded-full opacity-40 cursor-not-allowed"
          >
            <Image size={20} />
          </button>
          <button
            disabled
            title="Drawing (coming v0.3)"
            className="p-2 rounded-full opacity-40 cursor-not-allowed"
          >
            <Paintbrush size={20} />
          </button>
        </div>
      </div>
    </div>
  );
}
