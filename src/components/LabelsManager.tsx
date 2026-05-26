import { useEffect, useState } from "react";
import { X, Plus, Check, Trash2, Pencil } from "lucide-react";
import { useStore } from "../store";
import { api } from "../api";

export function LabelsManager() {
  const { labelsManagerOpen, closeLabelsManager, labels, load, showToast } =
    useStore();
  const [newName, setNewName] = useState("");
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editingText, setEditingText] = useState("");

  useEffect(() => {
    if (!labelsManagerOpen) {
      setNewName("");
      setEditingId(null);
    }
  }, [labelsManagerOpen]);

  if (!labelsManagerOpen) return null;

  const create = async () => {
    const name = newName.trim();
    if (!name) return;
    try {
      await api.createLabel(name);
      setNewName("");
      await load();
    } catch (e) {
      showToast("Could not create label: " + String(e));
    }
  };

  const saveRename = async () => {
    if (!editingId) return;
    const name = editingText.trim();
    if (!name) {
      setEditingId(null);
      return;
    }
    await api.renameLabel(editingId, name);
    setEditingId(null);
    await load();
  };

  const remove = async (id: string) => {
    if (!confirm("Delete this label? Notes using it will keep their text.")) return;
    await api.deleteLabel(id);
    await load();
  };

  return (
    <div
      className="fixed inset-0 z-50 modal-backdrop grid place-items-center p-4"
      onClick={closeLabelsManager}
    >
      <div
        className="w-full max-w-md rounded-lg shadow-keep-hover bg-white dark:bg-[#2d2e30] text-gray-800 dark:text-gray-100"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between px-5 py-3 border-b border-gray-200 dark:border-[#5f6368]">
          <h2 className="text-base font-medium">Edit labels</h2>
          <button
            onClick={closeLabelsManager}
            className="p-2 rounded-full hover:bg-black/5 dark:hover:bg-white/10"
          >
            <X size={18} />
          </button>
        </div>
        <div className="px-3 py-2">
          <div className="flex items-center gap-1 px-2 py-1.5 border-b border-gray-200 dark:border-[#5f6368]">
            <Plus size={16} className="opacity-60" />
            <input
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") create();
              }}
              placeholder="Create new label"
              className="flex-1 bg-transparent outline-none text-sm px-1"
            />
            {newName && (
              <button
                onClick={create}
                className="p-1 rounded hover:bg-black/5 dark:hover:bg-white/10"
              >
                <Check size={16} />
              </button>
            )}
          </div>
          <div className="max-h-80 overflow-y-auto py-1">
            {labels.length === 0 && (
              <div className="text-sm opacity-60 px-2 py-2">No labels yet.</div>
            )}
            {labels.map((l) => (
              <div
                key={l.id}
                className="group flex items-center gap-1 px-2 py-1 rounded hover:bg-black/5 dark:hover:bg-white/10"
              >
                {editingId === l.id ? (
                  <>
                    <input
                      autoFocus
                      value={editingText}
                      onChange={(e) => setEditingText(e.target.value)}
                      onKeyDown={(e) => {
                        if (e.key === "Enter") saveRename();
                        else if (e.key === "Escape") setEditingId(null);
                      }}
                      className="flex-1 bg-transparent outline-none border-b border-gray-300 dark:border-[#5f6368] text-sm px-1 py-0.5"
                    />
                    <button
                      onClick={saveRename}
                      className="p-1 rounded hover:bg-black/10 dark:hover:bg-white/20"
                    >
                      <Check size={16} />
                    </button>
                  </>
                ) : (
                  <>
                    <span className="flex-1 text-sm truncate px-1">{l.name}</span>
                    <button
                      onClick={() => {
                        setEditingId(l.id);
                        setEditingText(l.name);
                      }}
                      className="opacity-0 group-hover:opacity-100 p-1 rounded hover:bg-black/10 dark:hover:bg-white/20"
                      title="Rename"
                    >
                      <Pencil size={14} />
                    </button>
                    <button
                      onClick={() => remove(l.id)}
                      className="opacity-0 group-hover:opacity-100 p-1 rounded hover:bg-black/10 dark:hover:bg-white/20"
                      title="Delete"
                    >
                      <Trash2 size={14} />
                    </button>
                  </>
                )}
              </div>
            ))}
          </div>
        </div>
        <div className="px-5 py-3 border-t border-gray-200 dark:border-[#5f6368] text-right">
          <button
            onClick={closeLabelsManager}
            className="px-4 py-1.5 text-sm font-medium rounded hover:bg-black/5 dark:hover:bg-white/10"
          >
            Done
          </button>
        </div>
      </div>
    </div>
  );
}
