import { useEffect, useMemo, useState } from "react";
import { TopBar } from "./components/TopBar";
import { Sidebar } from "./components/Sidebar";
import { NoteGrid } from "./components/NoteGrid";
import { NewNoteBar } from "./components/NewNoteBar";
import { NoteEditor } from "./components/NoteEditor";
import { SettingsModal } from "./components/SettingsModal";
import { LabelsManager } from "./components/LabelsManager";
import { useStore } from "./store";
import { api } from "./api";
import { filterNotes } from "./lib/filterNotes";
import { Lightbulb, Archive, Trash2, Tag } from "lucide-react";

export default function App() {
  const { notes, labels, section, search, load, toast, showToast } = useStore();
  const [sidebarExpanded, setSidebarExpanded] = useState(true);

  const emptyTrash = async () => {
    if (!confirm("Permanently delete all notes in Trash?")) return;
    await api.emptyTrash();
    await load();
    showToast("Trash emptied");
  };

  useEffect(() => {
    load();
  }, [load]);

  const filtered = useMemo(
    () => filterNotes(notes, section, search),
    [notes, section, search],
  );

  const pinned = filtered.filter((n) => n.pinned && section.kind === "notes");
  const others = section.kind === "notes" ? filtered.filter((n) => !n.pinned) : filtered;

  const showNewBar = section.kind === "notes" && !search.trim();

  const headerLabel = (() => {
    if (section.kind === "label") {
      const l = labels.find((x) => x.id === section.labelId);
      return l?.name || "Label";
    }
    if (section.kind === "archive") return "Archive";
    if (section.kind === "trash") return "Trash";
    return "Notes";
  })();

  return (
    <div className="h-full flex flex-col bg-white dark:bg-[#202124] text-gray-800 dark:text-gray-100">
      <TopBar onMenu={() => setSidebarExpanded((v) => !v)} />
      <div className="flex-1 min-h-0 flex">
        <Sidebar expanded={sidebarExpanded} />
        <main className="flex-1 min-w-0 overflow-y-auto px-4 sm:px-8 py-6">
          {showNewBar && <NewNoteBar />}

          {section.kind === "trash" && (
            <div className="max-w-5xl mx-auto mb-4 flex items-center justify-between">
              <p className="text-sm text-gray-600 dark:text-gray-400">
                Notes in Trash can be restored or deleted forever.
              </p>
              {filtered.length > 0 && (
                <button
                  onClick={emptyTrash}
                  className="text-sm px-3 py-1.5 rounded border border-gray-300 dark:border-[#5f6368] hover:bg-black/5 dark:hover:bg-white/10"
                >
                  Empty Trash
                </button>
              )}
            </div>
          )}

          {filtered.length === 0 ? (
            <EmptyState section={section.kind} headerLabel={headerLabel} />
          ) : (
            <div className="max-w-[1600px] mx-auto">
              {section.kind === "notes" && pinned.length > 0 && (
                <>
                  <SectionLabel text="PINNED" />
                  <NoteGrid notes={pinned} />
                  {others.length > 0 && <SectionLabel text="OTHERS" />}
                </>
              )}
              {others.length > 0 && <NoteGrid notes={others} />}
            </div>
          )}
        </main>
      </div>

      <NoteEditor />
      <SettingsModal />
      <LabelsManager />

      {toast && (
        <div className="fixed left-1/2 -translate-x-1/2 bottom-6 z-50 px-4 py-2 rounded bg-[#3c4043] text-white text-sm shadow-lg">
          {toast}
        </div>
      )}
    </div>
  );
}

function SectionLabel({ text }: { text: string }) {
  return (
    <div className="text-[11px] font-medium tracking-widest text-gray-500 dark:text-gray-400 px-2 mt-2 mb-2 select-none">
      {text}
    </div>
  );
}

function EmptyState({
  section,
  headerLabel,
}: {
  section: "notes" | "archive" | "trash" | "label";
  headerLabel: string;
}) {
  const map: Record<string, { icon: React.ReactNode; text: string }> = {
    notes: {
      icon: <Lightbulb size={120} strokeWidth={1.2} />,
      text: "Notes you add appear here",
    },
    archive: {
      icon: <Archive size={120} strokeWidth={1.2} />,
      text: "Your archived notes appear here",
    },
    trash: {
      icon: <Trash2 size={120} strokeWidth={1.2} />,
      text: "No notes in Trash",
    },
    label: {
      icon: <Tag size={120} strokeWidth={1.2} />,
      text: `No notes with label "${headerLabel}"`,
    },
  };
  const { icon, text } = map[section];
  return (
    <div className="flex flex-col items-center justify-center text-gray-400 dark:text-gray-500 mt-20">
      {icon}
      <div className="mt-4 text-lg">{text}</div>
    </div>
  );
}
