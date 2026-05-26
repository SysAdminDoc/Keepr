import { useEffect, useRef, useState } from "react";
import { X, Plus, Check, Trash2, Pencil } from "lucide-react";
import { useStore } from "../store";
import { api } from "../api";
import { useEscape } from "../hooks/useEscape";
import { useFocusTrap } from "../hooks/useFocusTrap";
import { ConfirmDialog } from "./ConfirmDialog";

export function LabelsManager() {
  const labelsManagerOpen = useStore((s) => s.labelsManagerOpen);
  const closeLabelsManager = useStore((s) => s.closeLabelsManager);
  const labels = useStore((s) => s.labels);
  const showToast = useStore((s) => s.showToast);
  const upsertLabel = useStore((s) => s.upsertLabel);
  const patchLabel = useStore((s) => s.patchLabel);
  const removeLabel = useStore((s) => s.removeLabel);

  const [newName, setNewName] = useState("");
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editingText, setEditingText] = useState("");
  const [pendingDeleteId, setPendingDeleteId] = useState<string | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  useEscape(labelsManagerOpen && pendingDeleteId === null, closeLabelsManager);
  useFocusTrap(containerRef, labelsManagerOpen);

  useEffect(() => {
    if (!labelsManagerOpen) {
      setNewName("");
      setEditingId(null);
      setPendingDeleteId(null);
    }
  }, [labelsManagerOpen]);

  if (!labelsManagerOpen) return null;

  const create = async () => {
    const name = newName.trim();
    if (!name) return;
    try {
      const lbl = await api.createLabel(name);
      setNewName("");
      upsertLabel(lbl);
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
    try {
      await api.renameLabel(editingId, name);
      patchLabel(editingId, { name });
      setEditingId(null);
    } catch (e) {
      showToast("Could not rename label: " + String(e));
    }
  };

  const performDelete = async () => {
    if (!pendingDeleteId) return;
    const id = pendingDeleteId;
    setPendingDeleteId(null);
    try {
      await api.deleteLabel(id);
      removeLabel(id);
    } catch (e) {
      showToast("Could not delete label: " + String(e));
    }
  };

  return (
    <>
      <div
        className="fixed inset-0 z-50 modal-backdrop grid place-items-center p-4"
        onClick={closeLabelsManager}
        role="dialog"
        aria-modal="true"
        aria-labelledby="labels-title"
      >
        <div
          ref={containerRef}
          className="w-full max-w-md rounded-lg shadow-keep-hover bg-white dark:bg-[#2d2e30] text-gray-800 dark:text-gray-100"
          onClick={(e) => e.stopPropagation()}
        >
          <div className="flex items-center justify-between px-5 py-3 border-b border-gray-200 dark:border-[#5f6368]">
            <h2 id="labels-title" className="text-base font-medium">
              Edit labels
            </h2>
            <button
              onClick={closeLabelsManager}
              aria-label="Close labels manager"
              title="Close labels manager"
              className="p-2 rounded-full hover:bg-black/5 dark:hover:bg-white/10"
            >
              <X size={18} />
            </button>
          </div>
          <div className="px-3 py-2">
            <div className="flex items-center gap-1 px-2 py-1.5 border-b border-gray-200 dark:border-[#5f6368]">
              <Plus size={16} className="opacity-60" aria-hidden />
              <input
                value={newName}
                onChange={(e) => setNewName(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") create();
                }}
                placeholder="Create new label"
                aria-label="New label name"
                className="flex-1 bg-transparent outline-none text-sm px-1"
              />
              {newName && (
                <button
                  onClick={create}
                  aria-label="Save new label"
                  title="Save new label"
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
                          else if (e.key === "Escape") {
                            e.stopPropagation();
                            setEditingId(null);
                          }
                        }}
                        aria-label={`Rename label ${l.name}`}
                        className="flex-1 bg-transparent outline-none border-b border-gray-300 dark:border-[#5f6368] text-sm px-1 py-0.5"
                      />
                      <button
                        onClick={saveRename}
                        aria-label="Save rename"
                        title="Save rename"
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
                        aria-label={`Rename label ${l.name}`}
                        title="Rename"
                        className="opacity-0 group-hover:opacity-100 focus:opacity-100 p-1 rounded hover:bg-black/10 dark:hover:bg-white/20"
                      >
                        <Pencil size={14} />
                      </button>
                      <button
                        onClick={() => setPendingDeleteId(l.id)}
                        aria-label={`Delete label ${l.name}`}
                        title="Delete"
                        className="opacity-0 group-hover:opacity-100 focus:opacity-100 p-1 rounded hover:bg-black/10 dark:hover:bg-white/20"
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

      <ConfirmDialog
        open={pendingDeleteId !== null}
        title="Delete this label?"
        body="Notes that use this label will keep their text — the label will just be removed from them."
        confirmLabel="Delete"
        cancelLabel="Cancel"
        destructive
        onConfirm={performDelete}
        onCancel={() => setPendingDeleteId(null)}
      />
    </>
  );
}
