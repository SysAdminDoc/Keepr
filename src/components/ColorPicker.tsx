import clsx from "clsx";
import { Check, X, Ban } from "lucide-react";
import { COLOR_KEYS, COLOR_LABELS, LIGHT_HEX } from "../colors";
import type { BackgroundPatternKey, ColorKey } from "../types";
import {
  BACKGROUND_PATTERNS,
  BACKGROUND_PATTERN_LABELS,
  BACKGROUND_PATTERN_ORDER,
} from "../lib/backgroundPatterns";

interface Props {
  value: ColorKey;
  onChange: (c: ColorKey) => void;
  /** NF-22 — when set, the picker also renders Keep's 9 background
   *  patterns + a "no pattern" tile under the color row. Omitted when
   *  a host doesn't want to expose patterns (e.g. the card's quick
   *  palette where only color matters). */
  patternValue?: BackgroundPatternKey;
  onPatternChange?: (p: BackgroundPatternKey) => void;
  onClose?: () => void;
}

export function ColorPicker({
  value,
  onChange,
  patternValue,
  onPatternChange,
}: Props) {
  const showPatterns = onPatternChange !== undefined;
  return (
    <div
      className="p-2 bg-white dark:bg-[#2d2e30] rounded-lg shadow-lg border border-gray-200 dark:border-[#5f6368]"
      role="group"
      aria-label="Note color and background"
    >
      <div className="grid grid-cols-6 gap-1">
        {COLOR_KEYS.map((k) => {
          const selected = value === k;
          return (
            <button
              key={k}
              type="button"
              title={COLOR_LABELS[k]}
              aria-label={`${COLOR_LABELS[k]} background`}
              aria-pressed={selected}
              onClick={(e) => {
                e.stopPropagation();
                onChange(k);
              }}
              className={clsx(
                "w-8 h-8 rounded-full grid place-items-center transition-transform hover:scale-110 motion-reduce:transform-none motion-reduce:transition-none",
                k === "default"
                  ? "border border-gray-400 dark:border-gray-500"
                  : "border border-transparent",
                selected && "ring-2 ring-[#1a73e8] ring-offset-1",
              )}
              style={{ background: k === "default" ? "transparent" : LIGHT_HEX[k] }}
            >
              {k === "default" && selected ? (
                <Check size={16} className="text-gray-600" aria-hidden />
              ) : k === "default" ? (
                <X size={16} className="text-gray-600" aria-hidden />
              ) : selected ? (
                <Check size={16} className="text-gray-800" aria-hidden />
              ) : null}
            </button>
          );
        })}
      </div>
      {showPatterns && (
        <>
          <div className="my-2 h-px bg-gray-200 dark:bg-[#5f6368]" />
          <div
            className="grid grid-cols-5 gap-1"
            role="group"
            aria-label="Background pattern"
          >
            {BACKGROUND_PATTERN_ORDER.map((p) => {
              const selected = (patternValue ?? "") === p;
              const url = BACKGROUND_PATTERNS[p];
              return (
                <button
                  key={p || "none"}
                  type="button"
                  title={BACKGROUND_PATTERN_LABELS[p]}
                  aria-label={BACKGROUND_PATTERN_LABELS[p]}
                  aria-pressed={selected}
                  onClick={(e) => {
                    e.stopPropagation();
                    onPatternChange?.(p);
                  }}
                  className={clsx(
                    "w-12 h-10 rounded-md border border-gray-300 dark:border-[#5f6368] bg-white dark:bg-[#3c4043] overflow-hidden grid place-items-center hover:scale-105 transition-transform motion-reduce:transform-none motion-reduce:transition-none",
                    selected && "ring-2 ring-[#1a73e8] ring-offset-1",
                  )}
                  style={{
                    backgroundImage: url,
                    backgroundRepeat: "repeat",
                    backgroundSize: p === "" ? undefined : "32px",
                  }}
                >
                  {p === "" && (
                    <Ban size={16} className="text-gray-500" aria-hidden />
                  )}
                </button>
              );
            })}
          </div>
        </>
      )}
    </div>
  );
}
