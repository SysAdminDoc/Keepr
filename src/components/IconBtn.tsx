import clsx from "clsx";
import type { MouseEventHandler, ReactNode } from "react";

interface Props {
  ariaLabel: string;
  onClick?: MouseEventHandler<HTMLButtonElement>;
  children: ReactNode;
  /** Override default p-2 hover-tinted button styling. */
  className?: string;
  disabled?: boolean;
  pressed?: boolean;
}

/**
 * Shared icon-only button — replaces the 3 near-identical inline IconBtn
 * components scattered across NoteCard/NoteEditor (EI-31). Always carries
 * an aria-label and a matching title so screen-reader and hover-tooltip
 * users see the same name.
 */
export function IconBtn({
  ariaLabel,
  onClick,
  children,
  className,
  disabled,
  pressed,
}: Props) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-label={ariaLabel}
      aria-pressed={pressed}
      title={ariaLabel}
      disabled={disabled}
      className={clsx(
        "p-2 rounded-full hover:bg-black/10 dark:hover:bg-white/10 disabled:opacity-40 disabled:cursor-not-allowed",
        className,
      )}
    >
      {children}
    </button>
  );
}
