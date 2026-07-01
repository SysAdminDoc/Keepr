import { useCallback, useEffect, useRef, useState } from "react";
import {
  X,
  Camera,
  Upload,
  RotateCcw,
  Save,
  ScanLine,
  ChevronRight,
  ArrowLeft,
} from "lucide-react";
import { useEscape } from "../hooks/useEscape";
import { useFocusTrap } from "../hooks/useFocusTrap";

interface Props {
  open: boolean;
  onCancel: () => void;
  onSave: (pngBytes: number[]) => Promise<void> | void;
}

type Phase = "source" | "camera" | "crop" | "preview";
type FilterMode = "color" | "enhanced" | "grayscale" | "bw";
interface Point {
  x: number;
  y: number;
}

/* ── OpenCV lazy singleton ──────────────────────────────────────── */

// eslint-disable-next-line @typescript-eslint/no-explicit-any
let _cv: any = null;
// eslint-disable-next-line @typescript-eslint/no-explicit-any
let _cvPending: Promise<any> | null = null;

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function loadCV(): Promise<any> {
  if (_cv?.Mat) return Promise.resolve(_cv);
  if (_cvPending) return _cvPending;
  _cvPending = import("@techstark/opencv-js")
    .then((mod) => {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const cv: any = mod.default ?? mod;
      if (cv.Mat) {
        _cv = cv;
        return cv;
      }
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      return new Promise<any>((resolve, reject) => {
        const timeout = setTimeout(() => {
          reject(new Error("OpenCV WASM init timed out"));
        }, 30000);
        cv.onRuntimeInitialized = () => {
          clearTimeout(timeout);
          _cv = cv;
          resolve(cv);
        };
      });
    })
    .catch((e) => {
      _cvPending = null;
      throw e;
    });
  return _cvPending;
}

/* ── Geometry helpers ───────────────────────────────────────────── */

function ptDist(a: Point, b: Point) {
  return Math.hypot(a.x - b.x, a.y - b.y);
}

function orderCW(pts: Point[]): [Point, Point, Point, Point] {
  const s = [...pts].sort((a, b) => a.y - b.y);
  const top = s.slice(0, 2).sort((a, b) => a.x - b.x);
  const bot = s.slice(2, 4).sort((a, b) => a.x - b.x);
  return [top[0], top[1], bot[1], bot[0]];
}

function defaultCorners(w: number, h: number): [Point, Point, Point, Point] {
  const m = 0.05;
  return [
    { x: w * m, y: h * m },
    { x: w * (1 - m), y: h * m },
    { x: w * (1 - m), y: h * (1 - m) },
    { x: w * m, y: h * (1 - m) },
  ];
}

/* ── OpenCV image operations ────────────────────────────────────── */

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function detectEdges(cv: any, canvas: HTMLCanvasElement): Point[] | null {
  const src = cv.imread(canvas);
  const gray = new cv.Mat();
  const blur = new cv.Mat();
  const edges = new cv.Mat();
  const contours = new cv.MatVector();
  const hier = new cv.Mat();
  try {
    cv.cvtColor(src, gray, cv.COLOR_RGBA2GRAY);
    cv.GaussianBlur(gray, blur, new cv.Size(5, 5), 0);
    cv.Canny(blur, edges, 50, 150);
    const k = cv.getStructuringElement(cv.MORPH_RECT, new cv.Size(3, 3));
    cv.dilate(edges, edges, k);
    k.delete();
    cv.findContours(
      edges,
      contours,
      hier,
      cv.RETR_EXTERNAL,
      cv.CHAIN_APPROX_SIMPLE,
    );
    const areas: { i: number; a: number }[] = [];
    for (let i = 0; i < contours.size(); i++)
      areas.push({ i, a: cv.contourArea(contours.get(i)) });
    areas.sort((a, b) => b.a - a.a);
    for (const { i } of areas.slice(0, 5)) {
      const c = contours.get(i);
      const peri = cv.arcLength(c, true);
      const approx = new cv.Mat();
      cv.approxPolyDP(c, approx, 0.02 * peri, true);
      if (approx.rows === 4) {
        const pts: Point[] = [];
        for (let r = 0; r < 4; r++)
          pts.push({
            x: approx.data32S[r * 2],
            y: approx.data32S[r * 2 + 1],
          });
        approx.delete();
        if (cv.contourArea(c) / (src.rows * src.cols) > 0.1) return pts;
      } else {
        approx.delete();
      }
    }
    return null;
  } finally {
    src.delete();
    gray.delete();
    blur.delete();
    edges.delete();
    contours.delete();
    hier.delete();
  }
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function warpDoc(
  cv: any,
  canvas: HTMLCanvasElement,
  corners: [Point, Point, Point, Point],
): HTMLCanvasElement {
  const [tl, tr, br, bl] = corners;
  const w = Math.max(
    1,
    Math.min(
      4096,
      Math.round(Math.max(ptDist(tl, tr), ptDist(bl, br))),
    ),
  );
  const h = Math.max(
    1,
    Math.min(
      4096,
      Math.round(Math.max(ptDist(tl, bl), ptDist(tr, br))),
    ),
  );
  const src = cv.imread(canvas);
  const srcP = cv.matFromArray(4, 1, cv.CV_32FC2, [
    tl.x, tl.y, tr.x, tr.y, br.x, br.y, bl.x, bl.y,
  ]);
  const dstP = cv.matFromArray(4, 1, cv.CV_32FC2, [
    0, 0, w, 0, w, h, 0, h,
  ]);
  const M = cv.getPerspectiveTransform(srcP, dstP);
  const dst = new cv.Mat();
  cv.warpPerspective(src, dst, M, new cv.Size(w, h));
  const out = document.createElement("canvas");
  out.width = w;
  out.height = h;
  cv.imshow(out, dst);
  src.delete();
  srcP.delete();
  dstP.delete();
  M.delete();
  dst.delete();
  return out;
}

function simpleCrop(
  canvas: HTMLCanvasElement,
  corners: [Point, Point, Point, Point],
): HTMLCanvasElement {
  const xs = corners.map((c) => c.x);
  const ys = corners.map((c) => c.y);
  const x = Math.max(0, Math.round(Math.min(...xs)));
  const y = Math.max(0, Math.round(Math.min(...ys)));
  const w = Math.max(
    1,
    Math.min(canvas.width - x, Math.round(Math.max(...xs) - x)),
  );
  const h = Math.max(
    1,
    Math.min(canvas.height - y, Math.round(Math.max(...ys) - y)),
  );
  const out = document.createElement("canvas");
  out.width = w;
  out.height = h;
  out.getContext("2d")!.drawImage(canvas, x, y, w, h, 0, 0, w, h);
  return out;
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function applyFilter(
  cv: any,
  canvas: HTMLCanvasElement,
  mode: FilterMode,
): HTMLCanvasElement {
  if (mode === "color") {
    const c = document.createElement("canvas");
    c.width = canvas.width;
    c.height = canvas.height;
    c.getContext("2d")!.drawImage(canvas, 0, 0);
    return c;
  }
  const src = cv.imread(canvas);
  const gray = new cv.Mat();
  const dst = new cv.Mat();
  const out = document.createElement("canvas");
  out.width = canvas.width;
  out.height = canvas.height;
  try {
    cv.cvtColor(src, gray, cv.COLOR_RGBA2GRAY);
    if (mode === "grayscale") {
      cv.imshow(out, gray);
    } else if (mode === "bw") {
      cv.threshold(
        gray,
        dst,
        0,
        255,
        cv.THRESH_BINARY + cv.THRESH_OTSU,
      );
      cv.imshow(out, dst);
    } else {
      cv.adaptiveThreshold(
        gray,
        dst,
        255,
        cv.ADAPTIVE_THRESH_GAUSSIAN_C,
        cv.THRESH_BINARY,
        21,
        10,
      );
      cv.imshow(out, dst);
    }
    return out;
  } finally {
    src.delete();
    gray.delete();
    dst.delete();
  }
}

const FILTERS: { key: FilterMode; label: string }[] = [
  { key: "color", label: "Color" },
  { key: "enhanced", label: "Enhanced" },
  { key: "grayscale", label: "Gray" },
  { key: "bw", label: "B & W" },
];

/* ── Component ──────────────────────────────────────────────────── */

export function DocumentScannerModal({ open, onCancel, onSave }: Props) {
  const containerRef = useRef<HTMLDivElement>(null);
  useEscape(open, onCancel);
  useFocusTrap(containerRef, open);

  const [phase, setPhase] = useState<Phase>("source");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [cvOk, setCvOk] = useState(false);

  const srcRef = useRef<HTMLCanvasElement | null>(null);
  const [corners, setCorners] = useState<[Point, Point, Point, Point]>(
    defaultCorners(100, 100),
  );
  const warpedRef = useRef<HTMLCanvasElement | null>(null);

  const cropCanvasRef = useRef<HTMLCanvasElement>(null);
  const previewCanvasRef = useRef<HTMLCanvasElement>(null);
  const [dragging, setDragging] = useState(-1);
  const [filter, setFilter] = useState<FilterMode>("enhanced");

  const videoRef = useRef<HTMLVideoElement>(null);
  const streamRef = useRef<MediaStream | null>(null);
  const closedRef = useRef(false);
  const fileRef = useRef<HTMLInputElement>(null);
  const [dropActive, setDropActive] = useState(false);

  /* ── Load OpenCV on open ──────────────────────────────────────── */
  useEffect(() => {
    if (!open) return;
    closedRef.current = false;
    let cancelled = false;
    loadCV()
      .then(() => {
        if (!cancelled) setCvOk(true);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [open]);

  /* ── Reset on close ───────────────────────────────────────────── */
  useEffect(() => {
    if (open) return;
    closedRef.current = true;
    streamRef.current?.getTracks().forEach((t) => t.stop());
    streamRef.current = null;
    setPhase("source");
    setCorners(defaultCorners(100, 100));
    setFilter("enhanced");
    setBusy(false);
    setError(null);
    setDragging(-1);
    srcRef.current = null;
    warpedRef.current = null;
  }, [open]);

  /* ── Stop camera when leaving camera phase ────────────────────── */
  useEffect(() => {
    if (phase !== "camera") {
      streamRef.current?.getTracks().forEach((t) => t.stop());
      streamRef.current = null;
    }
  }, [phase]);

  /* ── Source loading ───────────────────────────────────────────── */

  const loadSource = useCallback(
    async (canvas: HTMLCanvasElement) => {
      srcRef.current = canvas;
      let detected: Point[] | null = null;
      if (cvOk && _cv) {
        try {
          detected = detectEdges(_cv, canvas);
        } catch {
          /* fallback to default corners */
        }
      }
      setCorners(
        detected
          ? orderCW(detected)
          : defaultCorners(canvas.width, canvas.height),
      );
      setPhase("crop");
    },
    [cvOk],
  );

  const startCamera = useCallback(async () => {
    setError(null);
    if (!navigator.mediaDevices?.getUserMedia) {
      setError(
        "Camera not available in this environment",
      );
      return;
    }
    try {
      const stream = await navigator.mediaDevices.getUserMedia({
        video: {
          facingMode: { ideal: "environment" },
          width: { ideal: 1920 },
          height: { ideal: 1080 },
        },
      });
      if (closedRef.current) {
        stream.getTracks().forEach((t) => t.stop());
        return;
      }
      streamRef.current = stream;
      setPhase("camera");
      requestAnimationFrame(() => {
        if (videoRef.current) {
          videoRef.current.srcObject = stream;
          videoRef.current.play().catch(() => {});
        }
      });
    } catch (e) {
      const err = e instanceof DOMException ? e : null;
      if (err?.name === "NotAllowedError")
        setError(
          "Camera access denied — check Windows Privacy → Camera",
        );
      else if (err?.name === "NotFoundError")
        setError("No camera found");
      else if (err?.name === "NotReadableError")
        setError("Camera is in use by another app");
      else setError("Could not access camera: " + String(e));
    }
  }, []);

  const captureFrame = useCallback(() => {
    const v = videoRef.current;
    if (!v || !v.videoWidth) return;
    const c = document.createElement("canvas");
    c.width = v.videoWidth;
    c.height = v.videoHeight;
    c.getContext("2d")!.drawImage(v, 0, 0);
    loadSource(c);
  }, [loadSource]);

  const handleFile = useCallback(
    (file: File) => {
      if (!file.type.startsWith("image/")) return;
      setError(null);
      const img = new Image();
      const url = URL.createObjectURL(file);
      img.onload = () => {
        const c = document.createElement("canvas");
        c.width = img.naturalWidth;
        c.height = img.naturalHeight;
        c.getContext("2d")!.drawImage(img, 0, 0);
        URL.revokeObjectURL(url);
        loadSource(c);
      };
      img.onerror = () => {
        URL.revokeObjectURL(url);
        setError("Could not load image");
      };
      img.src = url;
    },
    [loadSource],
  );

  /* ── Crop overlay drawing ─────────────────────────────────────── */

  const drawCrop = useCallback(() => {
    const display = cropCanvasRef.current;
    const src = srcRef.current;
    if (!display || !src) return;
    const ctx = display.getContext("2d");
    if (!ctx) return;

    const dpr = window.devicePixelRatio || 1;
    const rect = display.getBoundingClientRect();
    if (rect.width === 0 || rect.height === 0) return;
    display.width = Math.round(rect.width * dpr);
    display.height = Math.round(rect.height * dpr);
    ctx.scale(dpr, dpr);

    const scale = Math.min(rect.width / src.width, rect.height / src.height);
    const ox = (rect.width - src.width * scale) / 2;
    const oy = (rect.height - src.height * scale) / 2;

    ctx.drawImage(src, ox, oy, src.width * scale, src.height * scale);

    const dp = corners.map((p) => ({
      x: p.x * scale + ox,
      y: p.y * scale + oy,
    }));

    ctx.fillStyle = "rgba(0,0,0,0.35)";
    ctx.beginPath();
    ctx.rect(0, 0, rect.width, rect.height);
    ctx.moveTo(dp[0].x, dp[0].y);
    ctx.lineTo(dp[3].x, dp[3].y);
    ctx.lineTo(dp[2].x, dp[2].y);
    ctx.lineTo(dp[1].x, dp[1].y);
    ctx.closePath();
    ctx.fill("evenodd");

    ctx.strokeStyle = "#4285f4";
    ctx.lineWidth = 2;
    ctx.setLineDash([]);
    ctx.beginPath();
    ctx.moveTo(dp[0].x, dp[0].y);
    for (let i = 1; i < 4; i++) ctx.lineTo(dp[i].x, dp[i].y);
    ctx.closePath();
    ctx.stroke();

    for (const p of dp) {
      ctx.beginPath();
      ctx.arc(p.x, p.y, 12, 0, Math.PI * 2);
      ctx.fillStyle = "#4285f4";
      ctx.fill();
      ctx.beginPath();
      ctx.arc(p.x, p.y, 7, 0, Math.PI * 2);
      ctx.fillStyle = "#fff";
      ctx.fill();
    }
  }, [corners]);

  useEffect(() => {
    if (phase !== "crop") return;
    const raf = requestAnimationFrame(drawCrop);
    return () => cancelAnimationFrame(raf);
  }, [phase, corners, drawCrop]);

  /* ── Preview drawing ──────────────────────────────────────────── */

  const drawPreview = useCallback(() => {
    const canvas = previewCanvasRef.current;
    const warped = warpedRef.current;
    if (!canvas || !warped) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    let filtered: HTMLCanvasElement;
    try {
      if (cvOk && _cv && filter !== "color") {
        filtered = applyFilter(_cv, warped, filter);
      } else {
        filtered = document.createElement("canvas");
        filtered.width = warped.width;
        filtered.height = warped.height;
        filtered.getContext("2d")!.drawImage(warped, 0, 0);
      }
    } catch {
      filtered = document.createElement("canvas");
      filtered.width = warped.width;
      filtered.height = warped.height;
      filtered.getContext("2d")!.drawImage(warped, 0, 0);
    }

    const dpr = window.devicePixelRatio || 1;
    const rect = canvas.getBoundingClientRect();
    if (rect.width === 0 || rect.height === 0) return;
    canvas.width = Math.round(rect.width * dpr);
    canvas.height = Math.round(rect.height * dpr);
    ctx.scale(dpr, dpr);

    ctx.fillStyle = "#1a1a1a";
    ctx.fillRect(0, 0, rect.width, rect.height);

    const scale = Math.min(
      rect.width / filtered.width,
      rect.height / filtered.height,
    );
    const ox = (rect.width - filtered.width * scale) / 2;
    const oy = (rect.height - filtered.height * scale) / 2;
    ctx.drawImage(
      filtered,
      ox,
      oy,
      filtered.width * scale,
      filtered.height * scale,
    );
  }, [filter, cvOk]);

  useEffect(() => {
    if (phase !== "preview") return;
    const raf = requestAnimationFrame(drawPreview);
    return () => cancelAnimationFrame(raf);
  }, [phase, filter, drawPreview]);

  /* ── Crop pointer handlers ────────────────────────────────────── */

  const imgPt = useCallback(
    (e: React.PointerEvent): Point => {
      const d = cropCanvasRef.current!;
      const s = srcRef.current!;
      const r = d.getBoundingClientRect();
      const scale = Math.min(r.width / s.width, r.height / s.height);
      const ox = (r.width - s.width * scale) / 2;
      const oy = (r.height - s.height * scale) / 2;
      return {
        x: Math.max(
          0,
          Math.min(s.width, (e.clientX - r.left - ox) / scale),
        ),
        y: Math.max(
          0,
          Math.min(s.height, (e.clientY - r.top - oy) / scale),
        ),
      };
    },
    [],
  );

  const onPointerDown = useCallback(
    (e: React.PointerEvent<HTMLCanvasElement>) => {
      const d = cropCanvasRef.current!;
      const s = srcRef.current!;
      const r = d.getBoundingClientRect();
      const scale = Math.min(r.width / s.width, r.height / s.height);
      const ox = (r.width - s.width * scale) / 2;
      const oy = (r.height - s.height * scale) / 2;
      const mx = e.clientX - r.left;
      const my = e.clientY - r.top;
      let best = -1;
      let bestD = 30;
      for (let i = 0; i < 4; i++) {
        const dp = {
          x: corners[i].x * scale + ox,
          y: corners[i].y * scale + oy,
        };
        const dd = ptDist({ x: mx, y: my }, dp);
        if (dd < bestD) {
          bestD = dd;
          best = i;
        }
      }
      if (best >= 0) {
        e.currentTarget.setPointerCapture(e.pointerId);
        setDragging(best);
      }
    },
    [corners],
  );

  const onPointerMove = useCallback(
    (e: React.PointerEvent<HTMLCanvasElement>) => {
      if (dragging < 0) return;
      const p = imgPt(e);
      setCorners((prev) => {
        const next: [Point, Point, Point, Point] = [...prev];
        next[dragging] = p;
        return next;
      });
    },
    [dragging, imgPt],
  );

  const onPointerUp = useCallback(
    (e: React.PointerEvent<HTMLCanvasElement>) => {
      if (dragging >= 0) {
        e.currentTarget.releasePointerCapture(e.pointerId);
        setDragging(-1);
      }
    },
    [dragging],
  );

  /* ── Actions ──────────────────────────────────────────────────── */

  const doWarp = useCallback(async () => {
    const src = srcRef.current;
    if (!src) return;
    setBusy(true);
    setError(null);
    try {
      const warped =
        cvOk && _cv
          ? warpDoc(_cv, src, corners)
          : simpleCrop(src, corners);
      warpedRef.current = warped;
      setPhase("preview");
    } catch (e) {
      setError("Processing failed: " + String(e));
    } finally {
      setBusy(false);
    }
  }, [corners, cvOk]);

  const doSave = useCallback(async () => {
    const warped = warpedRef.current;
    if (!warped) return;
    setBusy(true);
    setError(null);
    try {
      let final: HTMLCanvasElement;
      if (cvOk && _cv && filter !== "color") {
        final = applyFilter(_cv, warped, filter);
      } else {
        final = document.createElement("canvas");
        final.width = warped.width;
        final.height = warped.height;
        final.getContext("2d")!.drawImage(warped, 0, 0);
      }
      const blob: Blob | null = await new Promise((resolve) =>
        final.toBlob((b) => resolve(b), "image/png"),
      );
      if (!blob) throw new Error("canvas toBlob returned null");
      const bytes = Array.from(new Uint8Array(await blob.arrayBuffer()));
      await onSave(bytes);
    } catch (e) {
      setError("Could not save: " + String(e));
    } finally {
      setBusy(false);
    }
  }, [filter, cvOk, onSave]);

  const redetect = useCallback(() => {
    const src = srcRef.current;
    if (!src || !cvOk || !_cv) return;
    try {
      const pts = detectEdges(_cv, src);
      setCorners(
        pts ? orderCW(pts) : defaultCorners(src.width, src.height),
      );
    } catch {
      /* keep current corners */
    }
  }, [cvOk]);

  const goBack = useCallback(() => {
    if (phase === "preview") setPhase("crop");
    else if (phase === "crop" || phase === "camera") setPhase("source");
  }, [phase]);

  /* ── Guard ────────────────────────────────────────────────────── */
  if (!open) return null;

  const btnBase =
    "inline-flex items-center gap-2 px-4 py-1.5 text-sm rounded disabled:opacity-50";
  const btnPrimary = `${btnBase} bg-[var(--keepr-accent)] text-white font-medium hover:bg-[var(--keepr-accent-hover)]`;
  const btnSecondary = `${btnBase} border border-gray-300 dark:border-[#5f6368] hover:bg-black/5 dark:hover:bg-white/10`;

  return (
    <div
      className="fixed inset-0 z-[58] modal-backdrop grid place-items-center p-4"
      onClick={onCancel}
      role="dialog"
      aria-modal="true"
      aria-labelledby="scanner-title"
    >
      <div
        ref={containerRef}
        className="w-full max-w-4xl max-h-[90vh] rounded-lg shadow-2xl bg-white dark:bg-[#2d2e30] text-gray-800 dark:text-gray-100 flex flex-col overflow-hidden"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex items-center justify-between px-5 py-3 border-b border-gray-200 dark:border-[#5f6368] shrink-0">
          <div className="flex items-center gap-2">
            {phase !== "source" && (
              <button
                type="button"
                onClick={goBack}
                className="p-1.5 rounded-full hover:bg-black/5 dark:hover:bg-white/10"
                aria-label="Go back"
              >
                <ArrowLeft size={16} aria-hidden />
              </button>
            )}
            <h2
              id="scanner-title"
              className="text-base font-medium flex items-center gap-2"
            >
              <ScanLine size={16} aria-hidden /> Document scanner
            </h2>
          </div>
          <button
            type="button"
            onClick={onCancel}
            aria-label="Close scanner"
            className="p-2 rounded-full hover:bg-black/5 dark:hover:bg-white/10"
          >
            <X size={18} aria-hidden />
          </button>
        </div>

        {/* Phase: source */}
        {phase === "source" && (
          <>
            <div
              className={
                "flex-1 flex flex-col items-center justify-center gap-5 p-8 min-h-[320px]" +
                (dropActive
                  ? " ring-2 ring-[var(--keepr-accent)] ring-inset"
                  : "")
              }
              onDragOver={(e) => {
                if (e.dataTransfer.types.includes("Files")) {
                  e.preventDefault();
                  setDropActive(true);
                }
              }}
              onDragLeave={() => setDropActive(false)}
              onDrop={(e) => {
                e.preventDefault();
                setDropActive(false);
                const f = e.dataTransfer.files[0];
                if (f) handleFile(f);
              }}
            >
              <ScanLine
                size={48}
                className="opacity-30"
                aria-hidden
              />
              <p className="text-sm opacity-60">
                Capture or choose an image to scan
              </p>
              <div className="flex gap-3">
                <button
                  type="button"
                  onClick={startCamera}
                  className={btnSecondary}
                >
                  <Camera size={16} aria-hidden /> Use camera
                </button>
                <button
                  type="button"
                  onClick={() => fileRef.current?.click()}
                  className={btnSecondary}
                >
                  <Upload size={16} aria-hidden /> Choose image
                </button>
                <input
                  ref={fileRef}
                  type="file"
                  accept="image/png,image/jpeg,image/gif,image/webp,image/bmp,image/tiff"
                  className="hidden"
                  onChange={(e) => {
                    const f = e.target.files?.[0];
                    if (f) handleFile(f);
                    e.target.value = "";
                  }}
                />
              </div>
              <p className="text-xs opacity-40">
                or drop an image here
              </p>
              {!cvOk && (
                <p className="text-xs opacity-40">
                  Loading image processor…
                </p>
              )}
              {error && (
                <p className="text-sm text-red-500 dark:text-red-400 text-center max-w-md">
                  {error}
                </p>
              )}
            </div>
          </>
        )}

        {/* Phase: camera */}
        {phase === "camera" && (
          <>
            <div className="flex-1 min-h-0 p-3 flex items-center justify-center bg-black">
              <video
                ref={videoRef}
                autoPlay
                playsInline
                muted
                className="max-w-full max-h-full rounded object-contain"
              />
            </div>
            <div className="flex items-center justify-end gap-2 px-5 py-3 border-t border-gray-200 dark:border-[#5f6368] shrink-0">
              <button
                type="button"
                onClick={goBack}
                className={`${btnBase} hover:bg-black/5 dark:hover:bg-white/10`}
              >
                Cancel
              </button>
              <button
                type="button"
                onClick={captureFrame}
                className={btnPrimary}
              >
                <Camera size={14} aria-hidden /> Capture
              </button>
            </div>
          </>
        )}

        {/* Phase: crop */}
        {phase === "crop" && (
          <>
            <div className="flex-1 min-h-0 p-3">
              <canvas
                ref={cropCanvasRef}
                className="w-full h-full touch-none"
                style={{ cursor: dragging >= 0 ? "grabbing" : "grab" }}
                onPointerDown={onPointerDown}
                onPointerMove={onPointerMove}
                onPointerUp={onPointerUp}
                onPointerCancel={onPointerUp}
                aria-label="Crop area — drag corners to frame document"
              />
            </div>
            <div className="flex items-center gap-2 px-5 py-3 border-t border-gray-200 dark:border-[#5f6368] shrink-0">
              <span className="text-xs opacity-60">
                Drag corners to frame the document
              </span>
              {cvOk && (
                <button
                  type="button"
                  onClick={redetect}
                  className={btnSecondary}
                >
                  <RotateCcw size={14} aria-hidden /> Auto-detect
                </button>
              )}
              <div className="flex-1" />
              {error && (
                <span className="text-xs text-red-500 dark:text-red-400">
                  {error}
                </span>
              )}
              <button
                type="button"
                onClick={doWarp}
                disabled={busy}
                className={btnPrimary}
              >
                {busy ? "Processing…" : "Next"}{" "}
                {!busy && <ChevronRight size={14} aria-hidden />}
              </button>
            </div>
          </>
        )}

        {/* Phase: preview */}
        {phase === "preview" && (
          <>
            <div className="flex-1 min-h-0 p-3">
              <canvas
                ref={previewCanvasRef}
                className="w-full h-full rounded"
                aria-label="Scan preview"
              />
            </div>
            <div className="flex items-center gap-2 px-5 py-3 border-t border-gray-200 dark:border-[#5f6368] shrink-0">
              <div
                className="flex gap-1"
                role="group"
                aria-label="Enhancement filter"
              >
                {FILTERS.map((f) => (
                  <button
                    key={f.key}
                    type="button"
                    onClick={() => setFilter(f.key)}
                    aria-pressed={filter === f.key}
                    className={
                      "px-3 py-1 text-sm rounded border transition-colors " +
                      (filter === f.key
                        ? "bg-[var(--keepr-accent)] text-white border-transparent"
                        : "border-gray-300 dark:border-[#5f6368] hover:bg-black/5 dark:hover:bg-white/10")
                    }
                  >
                    {f.label}
                  </button>
                ))}
              </div>
              {!cvOk && filter !== "color" && (
                <span className="text-xs opacity-40">
                  Filters unavailable
                </span>
              )}
              <div className="flex-1" />
              {error && (
                <span className="text-xs text-red-500 dark:text-red-400">
                  {error}
                </span>
              )}
              <button
                type="button"
                onClick={doSave}
                disabled={busy}
                className={btnPrimary}
              >
                <Save size={14} aria-hidden />{" "}
                {busy ? "Saving…" : "Save scan"}
              </button>
            </div>
          </>
        )}
      </div>
    </div>
  );
}
