import { useEffect, useRef, useState } from "react";
import {
  X,
  Eraser,
  Pencil,
  Undo2,
  Trash2,
  Save,
} from "lucide-react";
import { useEscape } from "../hooks/useEscape";
import { useFocusTrap } from "../hooks/useFocusTrap";

interface Props {
  open: boolean;
  onCancel: () => void;
  onSave: (pngBytes: number[]) => Promise<void> | void;
}

const PALETTE = [
  "#202124", // ink black
  "#d93025", // red
  "#f9ab00", // orange
  "#fbbc04", // yellow
  "#1e8e3e", // green
  "#1a73e8", // blue
  "#a142f4", // purple
  "#ffffff", // white (effective eraser on the off-white canvas, but
  //           we use a dedicated Eraser tool too for muscle memory)
];

const SIZES = [2, 5, 12];

interface Stroke {
  color: string;
  size: number;
  erase: boolean;
  points: { x: number; y: number }[];
}

/**
 * NF-V0.5-E — vector-ish drawing canvas. Strokes are tracked as point
 * arrays + color + size, then rasterised to a PNG on save. We don't
 * store the strokes themselves yet (the original research plan
 * mentioned SVG); raster-only keeps the attachment pipeline identical
 * to image-paste. Re-editing an existing drawing is intentionally out
 * of scope for the first cut — vector replay can land alongside the
 * SVG storage in a later release.
 *
 * Pen pressure is read off PointerEvent.pressure when the input
 * device reports it (Surface Pen, Wacom). Mouse devices report 0.5
 * so strokes look uniform there — same outcome as Keep on a non-pen
 * machine.
 */
export function DrawingCanvasModal({ open, onCancel, onSave }: Props) {
  const containerRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  useEscape(open, onCancel);
  useFocusTrap(containerRef, open);

  const [color, setColor] = useState<string>(PALETTE[0]);
  const [size, setSize] = useState<number>(SIZES[1]);
  const [erase, setErase] = useState<boolean>(false);
  const strokesRef = useRef<Stroke[]>([]);
  const drawingRef = useRef<Stroke | null>(null);
  const [hasStrokes, setHasStrokes] = useState<boolean>(false);
  const [busy, setBusy] = useState<boolean>(false);

  // Redraw the canvas from the stroke buffer whenever the modal
  // opens, the window resizes, or the user undoes a stroke.
  const repaint = () => {
    const c = canvasRef.current;
    if (!c) return;
    const ctx = c.getContext("2d");
    if (!ctx) return;
    ctx.save();
    ctx.fillStyle = "#fafafa";
    ctx.fillRect(0, 0, c.width, c.height);
    ctx.lineCap = "round";
    ctx.lineJoin = "round";
    for (const s of strokesRef.current) {
      drawStrokeOn(ctx, s);
    }
    const live = drawingRef.current;
    if (live) drawStrokeOn(ctx, live);
    ctx.restore();
  };

  useEffect(() => {
    if (!open) return;
    const c = canvasRef.current;
    if (!c) return;
    // Match the canvas's backing-store resolution to its CSS size so
    // strokes don't look blurry on retina / high-DPR displays.
    const dpr = window.devicePixelRatio || 1;
    const rect = c.getBoundingClientRect();
    c.width = Math.max(1, Math.round(rect.width * dpr));
    c.height = Math.max(1, Math.round(rect.height * dpr));
    const ctx = c.getContext("2d");
    if (ctx) ctx.scale(dpr, dpr);
    strokesRef.current = [];
    drawingRef.current = null;
    setHasStrokes(false);
    repaint();
  }, [open]);

  if (!open) return null;

  const localPoint = (e: React.PointerEvent<HTMLCanvasElement>) => {
    const rect = e.currentTarget.getBoundingClientRect();
    return { x: e.clientX - rect.left, y: e.clientY - rect.top };
  };

  const onPointerDown = (e: React.PointerEvent<HTMLCanvasElement>) => {
    e.currentTarget.setPointerCapture(e.pointerId);
    const p = localPoint(e);
    const pressureMult = e.pressure > 0 ? e.pressure / 0.5 : 1;
    drawingRef.current = {
      color,
      size: Math.max(1, size * pressureMult),
      erase,
      points: [p],
    };
    repaint();
  };
  const onPointerMove = (e: React.PointerEvent<HTMLCanvasElement>) => {
    const live = drawingRef.current;
    if (!live) return;
    live.points.push(localPoint(e));
    repaint();
  };
  const onPointerUp = (e: React.PointerEvent<HTMLCanvasElement>) => {
    e.currentTarget.releasePointerCapture(e.pointerId);
    const live = drawingRef.current;
    if (live && live.points.length > 0) {
      strokesRef.current.push(live);
      setHasStrokes(strokesRef.current.length > 0);
    }
    drawingRef.current = null;
    repaint();
  };

  const undo = () => {
    strokesRef.current.pop();
    setHasStrokes(strokesRef.current.length > 0);
    repaint();
  };
  const clearAll = () => {
    strokesRef.current = [];
    setHasStrokes(false);
    repaint();
  };

  const save = async () => {
    if (busy) return;
    const c = canvasRef.current;
    if (!c) return;
    setBusy(true);
    try {
      const blob: Blob | null = await new Promise((resolve) =>
        c.toBlob((b) => resolve(b), "image/png"),
      );
      if (!blob) throw new Error("canvas toBlob returned null");
      const buf = await blob.arrayBuffer();
      const bytes = Array.from(new Uint8Array(buf));
      await onSave(bytes);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div
      className="fixed inset-0 z-[58] modal-backdrop grid place-items-center p-4"
      onClick={onCancel}
      role="dialog"
      aria-modal="true"
      aria-labelledby="drawing-modal-title"
    >
      <div
        ref={containerRef}
        className="w-full max-w-3xl rounded-lg shadow-keep-hover bg-white dark:bg-[#2d2e30] text-gray-800 dark:text-gray-100 flex flex-col"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between px-5 py-3 border-b border-gray-200 dark:border-[#5f6368]">
          <h2
            id="drawing-modal-title"
            className="text-base font-medium flex items-center gap-2"
          >
            <Pencil size={16} aria-hidden /> New drawing
          </h2>
          <button
            type="button"
            onClick={onCancel}
            aria-label="Cancel drawing"
            className="p-2 rounded-full hover:bg-black/5 dark:hover:bg-white/10"
          >
            <X size={18} aria-hidden />
          </button>
        </div>

        <div className="flex flex-wrap items-center gap-3 px-5 py-2 border-b border-gray-200 dark:border-[#5f6368]">
          <div className="flex items-center gap-1" role="group" aria-label="Color">
            {PALETTE.map((c) => (
              <button
                key={c}
                type="button"
                onClick={() => {
                  setColor(c);
                  setErase(false);
                }}
                aria-label={`Color ${c}`}
                aria-pressed={!erase && color === c}
                className={
                  "w-6 h-6 rounded-full border border-gray-400 transition-transform hover:scale-110 motion-reduce:transform-none " +
                  (!erase && color === c ? "ring-2 ring-[var(--keepr-accent)]" : "")
                }
                style={{ background: c }}
              />
            ))}
          </div>
          <div className="flex items-center gap-1" role="group" aria-label="Stroke size">
            {SIZES.map((s) => (
              <button
                key={s}
                type="button"
                onClick={() => setSize(s)}
                aria-label={`Stroke size ${s}`}
                aria-pressed={size === s}
                className={
                  "w-8 h-8 rounded grid place-items-center hover:bg-black/5 dark:hover:bg-white/10 " +
                  (size === s ? "bg-black/10 dark:bg-white/10" : "")
                }
              >
                <span
                  className="rounded-full"
                  style={{
                    width: `${s + 2}px`,
                    height: `${s + 2}px`,
                    background: erase ? "#bdbdbd" : color,
                  }}
                />
              </button>
            ))}
          </div>
          <button
            type="button"
            onClick={() => setErase((v) => !v)}
            aria-pressed={erase}
            className={
              "inline-flex items-center gap-1 px-2 py-1 text-sm rounded border border-gray-300 dark:border-[#5f6368] hover:bg-black/5 dark:hover:bg-white/10 " +
              (erase ? "bg-black/10 dark:bg-white/10" : "")
            }
          >
            <Eraser size={14} aria-hidden /> Eraser
          </button>
          <div className="flex-1" />
          <button
            type="button"
            onClick={undo}
            disabled={!hasStrokes || busy}
            className="inline-flex items-center gap-1 px-2 py-1 text-sm rounded border border-gray-300 dark:border-[#5f6368] hover:bg-black/5 dark:hover:bg-white/10 disabled:opacity-50"
            aria-label="Undo last stroke"
          >
            <Undo2 size={14} aria-hidden /> Undo
          </button>
          <button
            type="button"
            onClick={clearAll}
            disabled={!hasStrokes || busy}
            className="inline-flex items-center gap-1 px-2 py-1 text-sm rounded border border-gray-300 dark:border-[#5f6368] hover:bg-black/5 dark:hover:bg-white/10 disabled:opacity-50"
            aria-label="Clear canvas"
          >
            <Trash2 size={14} aria-hidden /> Clear
          </button>
        </div>

        <div className="p-3">
          <canvas
            ref={canvasRef}
            onPointerDown={onPointerDown}
            onPointerMove={onPointerMove}
            onPointerUp={onPointerUp}
            onPointerCancel={onPointerUp}
            className="w-full h-[420px] bg-[#fafafa] rounded border border-gray-300 dark:border-[#5f6368] touch-none"
            aria-label="Drawing canvas"
          />
        </div>

        <div className="flex items-center justify-end gap-2 px-5 py-3 border-t border-gray-200 dark:border-[#5f6368]">
          <button
            type="button"
            onClick={onCancel}
            className="px-4 py-1.5 text-sm rounded hover:bg-black/5 dark:hover:bg-white/10"
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={save}
            disabled={busy || !hasStrokes}
            className="inline-flex items-center gap-2 px-4 py-1.5 text-sm rounded bg-[var(--keepr-accent)] text-white font-medium hover:bg-[var(--keepr-accent-hover)] disabled:opacity-50"
          >
            <Save size={14} aria-hidden /> {busy ? "Saving…" : "Save drawing"}
          </button>
        </div>
      </div>
    </div>
  );
}

function drawStrokeOn(ctx: CanvasRenderingContext2D, s: Stroke) {
  if (s.points.length === 0) return;
  ctx.save();
  ctx.lineCap = "round";
  ctx.lineJoin = "round";
  if (s.erase) {
    // Erasing == painting the canvas background color so the export
    // still flattens to a single PNG with no transparency artifacts.
    ctx.strokeStyle = "#fafafa";
    ctx.lineWidth = s.size * 3; // eraser feels bigger than ink
  } else {
    ctx.strokeStyle = s.color;
    ctx.lineWidth = s.size;
  }
  ctx.beginPath();
  ctx.moveTo(s.points[0].x, s.points[0].y);
  if (s.points.length === 1) {
    // Single tap — render as a filled dot so the user sees feedback.
    ctx.arc(s.points[0].x, s.points[0].y, s.size / 2, 0, Math.PI * 2);
    ctx.fillStyle = s.erase ? "#fafafa" : s.color;
    ctx.fill();
  } else {
    for (let i = 1; i < s.points.length; i++) {
      ctx.lineTo(s.points[i].x, s.points[i].y);
    }
    ctx.stroke();
  }
  ctx.restore();
}
