import { useMemo, useRef, useState } from "react";
import { Bell, BellOff, X, Clock, Repeat, Moon } from "lucide-react";
import { useEscape } from "../hooks/useEscape";
import { useFocusTrap } from "../hooks/useFocusTrap";
import type { RecurrenceRule } from "../types";

type RecurrenceChoice = "" | RecurrenceRule;

interface Props {
  open: boolean;
  /** When non-null, the picker shows "Remove reminder" + snooze buttons. */
  existingFireAt: string | null;
  existingRrule: string | null;
  onSet: (fireAtIso: string, rrule: string | null) => void;
  /** Snooze the currently-set reminder to the given ISO timestamp. */
  onSnooze: (untilIso: string) => void;
  onClear: () => void;
  onClose: () => void;
}

/**
 * NF-V0.5-A — quick-pick reminder modal. Presets mirror Keep (Later
 * today / Tomorrow morning / Next Monday), plus an optional recurrence
 * dropdown (None/Daily/Weekly/Monthly/Yearly) and a Snooze panel that
 * only appears while editing an existing reminder.
 */
export function ReminderPicker({
  open,
  existingFireAt,
  existingRrule,
  onSet,
  onSnooze,
  onClear,
  onClose,
}: Props) {
  const containerRef = useRef<HTMLDivElement>(null);
  useEscape(open, onClose);
  useFocusTrap(containerRef, open);

  const oneHourFromNow = useMemo(
    () => new Date(Date.now() + 60 * 60 * 1000),
    // Recompute on every open transition so a long-lived picker tab picks
    // up the new "now" reference. The closure doesn't read `open` directly,
    // but the dep is load-bearing for that refresh.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [open],
  );
  const defaultCustom = toLocalDatetimeInput(oneHourFromNow);
  const [custom, setCustom] = useState<string>(defaultCustom);
  const [recurrence, setRecurrence] = useState<RecurrenceChoice>(
    normalizeRrule(existingRrule),
  );
  const [snoozeCustom, setSnoozeCustom] = useState<string>(defaultCustom);

  if (!open) return null;

  const rruleForCommit = (): string | null => (recurrence === "" ? null : recurrence);

  const setAt = (d: Date) => {
    if (d.getTime() <= Date.now()) {
      // Bump to the next day if the chosen preset already passed today.
      d = new Date(d.getTime() + 24 * 60 * 60 * 1000);
    }
    onSet(d.toISOString(), rruleForCommit());
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
  const nextMonday = () => {
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

  const snoozeBy = (ms: number) => {
    const until = new Date(Date.now() + ms);
    onSnooze(until.toISOString());
  };
  const snoozeTomorrowMorning = () => {
    const d = new Date();
    d.setDate(d.getDate() + 1);
    d.setHours(8, 0, 0, 0);
    if (d.getTime() <= Date.now()) d.setDate(d.getDate() + 1);
    onSnooze(d.toISOString());
  };
  const submitSnoozeCustom = () => {
    const parsed = new Date(snoozeCustom);
    if (Number.isNaN(parsed.getTime())) return;
    if (parsed.getTime() <= Date.now()) return;
    onSnooze(parsed.toISOString());
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
          <PresetRow label="Next Monday" subtitle="8:00 AM" onClick={nextMonday} />
          <div className="pt-2 border-t border-gray-200 dark:border-[#5f6368]">
            <label
              htmlFor="reminder-recurrence"
              className="flex items-center gap-2 px-2 py-2 text-sm"
            >
              <Repeat size={14} aria-hidden />
              <span>Repeat</span>
            </label>
            <div className="px-2">
              <select
                id="reminder-recurrence"
                value={recurrence}
                onChange={(e) =>
                  setRecurrence(e.target.value as RecurrenceChoice)
                }
                className="w-full px-2 py-1.5 text-sm rounded border border-gray-300 dark:border-[#5f6368] bg-transparent"
              >
                <option value="">Does not repeat</option>
                <option value="FREQ=DAILY">Daily</option>
                <option value="FREQ=WEEKLY">Weekly</option>
                <option value="FREQ=MONTHLY">Monthly</option>
                <option value="FREQ=YEARLY">Yearly</option>
              </select>
            </div>
          </div>
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
                className="px-3 py-1.5 text-sm rounded bg-[var(--keepr-accent)] text-white hover:bg-[var(--keepr-accent-hover)]"
              >
                Set
              </button>
            </div>
          </div>
          {existingFireAt && (
            <div className="pt-2 border-t border-gray-200 dark:border-[#5f6368]">
              <label className="flex items-center gap-2 px-2 py-2 text-sm">
                <Moon size={14} aria-hidden />
                <span>Snooze</span>
              </label>
              <div className="grid grid-cols-3 gap-1 px-2">
                <SnoozeChip label="10 min" onClick={() => snoozeBy(10 * 60 * 1000)} />
                <SnoozeChip label="1 hour" onClick={() => snoozeBy(60 * 60 * 1000)} />
                <SnoozeChip label="Tomorrow" onClick={snoozeTomorrowMorning} />
              </div>
              <div className="flex items-center gap-2 px-2 pt-2">
                <input
                  type="datetime-local"
                  value={snoozeCustom}
                  onChange={(e) => setSnoozeCustom(e.target.value)}
                  aria-label="Custom snooze date and time"
                  className="flex-1 px-2 py-1.5 text-sm rounded border border-gray-300 dark:border-[#5f6368] bg-transparent"
                />
                <button
                  type="button"
                  onClick={submitSnoozeCustom}
                  className="px-3 py-1.5 text-sm rounded border border-gray-300 dark:border-[#5f6368] hover:bg-black/5 dark:hover:bg-white/10"
                >
                  Snooze
                </button>
              </div>
            </div>
          )}
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

function SnoozeChip({
  label,
  onClick,
}: {
  label: string;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="px-2 py-1.5 text-xs rounded border border-gray-300 dark:border-[#5f6368] hover:bg-black/5 dark:hover:bg-white/10"
    >
      {label}
    </button>
  );
}

/** Coerce a raw RRULE string into the dropdown's accepted values. Anything
 *  outside the whitelisted FREQ=* set falls back to "Does not repeat" so
 *  the UI doesn't accidentally re-commit a value the Rust side will reject. */
function normalizeRrule(raw: string | null): RecurrenceChoice {
  if (raw === "FREQ=DAILY") return "FREQ=DAILY";
  if (raw === "FREQ=WEEKLY") return "FREQ=WEEKLY";
  if (raw === "FREQ=MONTHLY") return "FREQ=MONTHLY";
  if (raw === "FREQ=YEARLY") return "FREQ=YEARLY";
  return "";
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
