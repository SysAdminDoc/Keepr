import { useRef, useState } from "react";
import { Bell, BellOff, X, Clock } from "lucide-react";
import { useEscape } from "../hooks/useEscape";
import { useFocusTrap } from "../hooks/useFocusTrap";

interface Props {
  open: boolean;
  /** When non-null, the picker shows a "Remove reminder" button. */
  existingFireAt: string | null;
  onSet: (fireAtIso: string) => void;
  onClear: () => void;
  onClose: () => void;
}

/**
 * NF-02 — quick-pick reminder modal. Mirrors Keep's preset rows
 * (Later today, Tomorrow morning, Next week) plus a custom datetime
 * input. v0.4 ships single-shot only; recurrence (RRULE) deferred.
 */
export function ReminderPicker({
  open,
  existingFireAt,
  onSet,
  onClear,
  onClose,
}: Props) {
  const containerRef = useRef<HTMLDivElement>(null);
  useEscape(open, onClose);
  useFocusTrap(containerRef, open);

  // Default custom field to one hour from now in the user's local zone.
  const oneHourFromNow = new Date(Date.now() + 60 * 60 * 1000);
  const defaultCustom = toLocalDatetimeInput(oneHourFromNow);
  const [custom, setCustom] = useState<string>(defaultCustom);

  if (!open) return null;

  const setAt = (d: Date) => {
    if (d.getTime() <= Date.now()) {
      // Bump to the next day if the chosen preset already passed today.
      d = new Date(d.getTime() + 24 * 60 * 60 * 1000);
    }
    onSet(d.toISOString());
  };

  const laterToday = () => {
    const d = new Date();
    d.setHours(18, 0, 0, 0);
    setAt(d);
  };
  const tomorrowMorning = () => {
    const d = new Date();
    d.setDate(d.getDate() + 1);
    d.setHours(8, 0, 0, 0);
    setAt(d);
  };
  const nextWeek = () => {
    const d = new Date();
    const days = ((1 - d.getDay() + 7) % 7) || 7; // next Monday
    d.setDate(d.getDate() + days);
    d.setHours(8, 0, 0, 0);
    setAt(d);
  };
  const submitCustom = () => {
    const parsed = new Date(custom);
    if (Number.isNaN(parsed.getTime())) return;
    setAt(parsed);
  };

  return (
    <div
      className="fixed inset-0 z-[55] modal-backdrop grid place-items-center p-4"
      onClick={onClose}
      role="dialog"
      aria-modal="true"
      aria-labelledby="reminder-picker-title"
    >
      <div
        ref={containerRef}
        className="w-full max-w-sm rounded-lg shadow-keep-hover bg-white dark:bg-[#2d2e30] text-gray-800 dark:text-gray-100"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between px-5 py-3 border-b border-gray-200 dark:border-[#5f6368]">
          <h2
            id="reminder-picker-title"
            className="text-base font-medium flex items-center gap-2"
          >
            <Bell size={16} aria-hidden /> Remind me
          </h2>
          <button
            type="button"
            onClick={onClose}
            aria-label="Close reminder picker"
            className="p-2 rounded-full hover:bg-black/5 dark:hover:bg-white/10"
          >
            <X size={18} aria-hidden />
          </button>
        </div>
        <div className="px-3 py-2 space-y-1">
          <PresetRow label="Later today" subtitle="6:00 PM" onClick={laterToday} />
          <PresetRow
            label="Tomorrow morning"
            subtitle="8:00 AM"
            onClick={tomorrowMorning}
          />
          <PresetRow label="Next Monday" subtitle="8:00 AM" onClick={nextWeek} />
          <div className="pt-2 border-t border-gray-200 dark:border-[#5f6368]">
            <label className="flex items-center gap-2 px-2 py-2 text-sm">
              <Clock size={14} aria-hidden />
              <span>Pick date &amp; time</span>
            </label>
            <div className="flex items-center gap-2 px-2">
              <input
                type="datetime-local"
                value={custom}
                onChange={(e) => setCustom(e.target.value)}
                aria-label="Custom reminder date and time"
                className="flex-1 px-2 py-1.5 text-sm rounded border border-gray-300 dark:border-[#5f6368] bg-transparent"
              />
              <button
                type="button"
                onClick={submitCustom}
                className="px-3 py-1.5 text-sm rounded bg-[#1a73e8] text-white hover:bg-[#1557b0]"
              >
                Set
              </button>
            </div>
          </div>
          {existingFireAt && (
            <div className="pt-2 border-t border-gray-200 dark:border-[#5f6368]">
              <button
                type="button"
                onClick={onClear}
                className="flex items-center gap-2 w-full px-2 py-2 text-sm text-[#d93025] hover:bg-black/5 dark:hover:bg-white/10 rounded"
              >
                <BellOff size={14} aria-hidden /> Remove reminder
              </button>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

function PresetRow({
  label,
  subtitle,
  onClick,
}: {
  label: string;
  subtitle: string;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="flex items-center justify-between w-full px-2 py-2 text-sm rounded hover:bg-black/5 dark:hover:bg-white/10"
    >
      <span>{label}</span>
      <span className="text-xs opacity-60">{subtitle}</span>
    </button>
  );
}

/** Format a Date into the local "yyyy-MM-ddTHH:mm" string the
 *  <input type="datetime-local"> wants. */
function toLocalDatetimeInput(d: Date): string {
  const pad = (n: number) => String(n).padStart(2, "0");
  return (
    `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}` +
    `T${pad(d.getHours())}:${pad(d.getMinutes())}`
  );
}
