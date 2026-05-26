import { useEffect, useRef } from "react";
import { useEscape } from "../hooks/useEscape";
import { useFocusTrap } from "../hooks/useFocusTrap";

interface Props {
  open: boolean;
  title: string;
  body?: string;
  confirmLabel?: string;
  cancelLabel?: string;
  destructive?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

/**
 * In-app replacement for `window.confirm()` (EI-20). Styled to match the
 * rest of the UI in both themes; traps focus; closes on Escape; auto-focuses
 * the cancel button (safer default for destructive prompts).
 */
export function ConfirmDialog({
  open,
  title,
  body,
  confirmLabel = "Confirm",
  cancelLabel = "Cancel",
  destructive = false,
  onConfirm,
  onCancel,
}: Props) {
  const containerRef = useRef<HTMLDivElement>(null);
  const cancelRef = useRef<HTMLButtonElement>(null);
  useEscape(open, onCancel);
  useFocusTrap(containerRef, open);

  useEffect(() => {
    if (open) requestAnimationFrame(() => cancelRef.current?.focus());
  }, [open]);

  if (!open) return null;
  return (
    <div
      className="fixed inset-0 z-[60] modal-backdrop grid place-items-center p-4"
      onClick={onCancel}
      role="dialog"
      aria-modal="true"
      aria-labelledby="confirm-dialog-title"
    >
      <div
        ref={containerRef}
        className="w-full max-w-sm rounded-lg shadow-keep-hover bg-white dark:bg-[#2d2e30] text-gray-800 dark:text-gray-100"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="px-5 py-4">
          <h2 id="confirm-dialog-title" className="text-base font-medium">
            {title}
          </h2>
          {body && (
            <p className="mt-2 text-sm text-gray-600 dark:text-gray-400">{body}</p>
          )}
        </div>
        <div className="flex items-center justify-end gap-2 px-4 py-3 border-t border-gray-200 dark:border-[#5f6368]">
          <button
            ref={cancelRef}
            onClick={onCancel}
            className="px-4 py-1.5 text-sm font-medium rounded hover:bg-black/5 dark:hover:bg-white/10"
          >
            {cancelLabel}
          </button>
          <button
            onClick={onConfirm}
            className={
              destructive
                ? "px-4 py-1.5 text-sm font-medium rounded text-white bg-[#d93025] hover:bg-[#b1271b]"
                : "px-4 py-1.5 text-sm font-medium rounded text-white bg-[var(--keepr-accent)] hover:bg-[var(--keepr-accent-hover)]"
            }
          >
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
