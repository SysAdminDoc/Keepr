import clsx from "clsx";
import { Check, X } from "lucide-react";
import { COLOR_KEYS, COLOR_LABELS, LIGHT_HEX } from "../colors";
import type { ColorKey } from "../types";

interface Props {
  value: ColorKey;
  onChange: (c: ColorKey) => void;
  onClose?: () => void;
}

export function ColorPicker({ value, onChange }: Props) {
  return (
    <div
      className="grid grid-cols-6 gap-1 p-2 bg-white dark:bg-[#2d2e30] rounded-lg shadow-lg border border-gray-200 dark:border-[#5f6368]"
      role="group"
      aria-label="Note color"
    >
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
  );
}
