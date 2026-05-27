import { X } from "lucide-react";
import { useMemo, useState } from "react";
import clsx from "clsx";
import type { Attachment } from "../types";
import { convertFileSrc } from "@tauri-apps/api/core";

interface Props {
  attachments: Attachment[];
  /** Editor mode shows a remove button on hover; card preview hides it. */
  editable?: boolean;
  onRemove?: (a: Attachment) => void;
  /** Cap rendering to N images and show "+M" overflow tile. Cards use
   *  4; the editor passes Infinity to show them all. */
  maxVisible?: number;
  /** When true, prefer the 480-px thumbnail (NF-V0.5-B) for each image
   *  instead of the original. Cards pass true; the editor passes false
   *  so users see full quality. */
  preferThumb?: boolean;
}

/**
 * NF-01 — render image attachments. Mirrors Keep's grid breakpoints:
 *   1 image  → full width
 *   2 images → 2 columns
 *   3 images → 1 large top + 2 below
 *   4+       → 2 x 2 with "+N" overflow tile when more
 *
 * Source URL goes through the keepr-resource:// protocol registered in
 * src-tauri/src/lib.rs, so the renderer never sees a file:// or absolute
 * path. Filename suffix is derived from MIME (jpg/png/gif/webp/svg).
 */
export function AttachmentGrid({
  attachments,
  editable,
  onRemove,
  maxVisible = Infinity,
  preferThumb = false,
}: Props) {
  if (attachments.length === 0) return null;
  // v0.20.3 — split audio (rendered as <audio> rows) from images (kept
  // in the existing mosaic). Audio renders ABOVE images so the controls
  // stay reachable when an image grid grows tall.
  const audios = attachments.filter((a) => a.kind === "audio");
  const images = attachments.filter((a) => a.kind !== "audio");

  const visible = images.slice(0, maxVisible);
  const overflow = Math.max(0, images.length - visible.length);
  const n = visible.length;

  // Tailwind classes per count to mirror Keep's mosaic.
  const gridClass = clsx(
    "grid gap-px overflow-hidden",
    n === 1 && "grid-cols-1",
    n === 2 && "grid-cols-2",
    n === 3 && "grid-cols-2 grid-rows-2",
    n >= 4 && "grid-cols-2 grid-rows-2",
  );

  return (
    <div>
      {audios.length > 0 && (
        <div className="flex flex-col gap-2 px-3 py-2 bg-black/5 dark:bg-white/5">
          {audios.map((a) => (
            <AudioRow
              key={a.id}
              attachment={a}
              editable={editable}
              onRemove={onRemove}
            />
          ))}
        </div>
      )}
      {n > 0 && (
        <div className={clsx("relative bg-black/5 dark:bg-white/5", gridClass)}>
          {visible.map((a, i) => {
            // For 3-image layout the first image spans both columns.
            const spanFull = n === 3 && i === 0;
            return (
              <AttachmentTile
                key={a.id}
                attachment={a}
                spanFull={spanFull}
                singleton={n === 1}
                inMosaic={n >= 2}
                editable={editable}
                overflow={overflow > 0 && i === visible.length - 1 ? overflow : 0}
                onRemove={onRemove}
                preferThumb={preferThumb}
              />
            );
          })}
        </div>
      )}
    </div>
  );
}

function AudioRow({
  attachment,
  editable,
  onRemove,
}: {
  attachment: Attachment;
  editable?: boolean;
  onRemove?: (a: Attachment) => void;
}) {
  const src = useMemo(
    () => convertFileSrc(srcForAttachment(attachment), "keepr-resource"),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [attachment.id, attachment.mime],
  );
  return (
    <div className="flex items-center gap-2">
      <audio
        controls
        src={src}
        className="flex-1 h-10"
        aria-label={attachment.filename}
      />
      {editable && onRemove && (
        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation();
            onRemove(attachment);
          }}
          aria-label={`Remove ${attachment.filename}`}
          title="Remove voice note"
          className="p-1 rounded-full bg-black/60 text-white hover:bg-black/80"
        >
          <X size={14} aria-hidden />
        </button>
      )}
    </div>
  );
}

interface TileProps {
  attachment: Attachment;
  spanFull: boolean;
  singleton: boolean;
  inMosaic: boolean;
  editable?: boolean;
  overflow: number;
  onRemove?: (a: Attachment) => void;
  preferThumb: boolean;
}

function AttachmentTile({
  attachment,
  spanFull,
  singleton,
  inMosaic,
  editable,
  overflow,
  onRemove,
  preferThumb,
}: TileProps) {
  // NF-V0.5-B — try the 480px thumbnail first when preferThumb is true.
  // If the file doesn't exist (older attachments from before the thumb
  // pipeline landed), the `<img>` onError swaps to the original.
  const [thumbFailed, setThumbFailed] = useState(false);
  const useThumb = preferThumb && !thumbFailed;
  // Memoise src — convertFileSrc returns a stable string per (id, ext)
  // but React would still re-evaluate the call every render otherwise.
  const src = useMemo(
    () =>
      convertFileSrc(
        useThumb ? thumbFilename(attachment) : srcForAttachment(attachment),
        "keepr-resource",
      ),
    // Keyed on the attachment fields that actually change the URL, not the
    // wrapper object itself (which gets a new identity on every store patch).
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [attachment.id, attachment.mime, useThumb],
  );
  return (
    <figure
      className={clsx(
        "relative group/att overflow-hidden",
        spanFull && "col-span-2",
        singleton && "max-h-[28rem]",
        inMosaic && "aspect-square",
      )}
    >
      <img
        src={src}
        alt={attachment.filename || "Attachment"}
        loading="lazy"
        draggable={false}
        onError={() => {
          // Thumbnail missing (pre-v0.5 upload) — fall back to original.
          if (useThumb) setThumbFailed(true);
        }}
        className="w-full h-full object-cover"
      />
      {/* Overflow indicator overlays the last visible image when there
          are more than `maxVisible`. */}
      {overflow > 0 && (
        <div className="absolute inset-0 bg-black/50 text-white grid place-items-center text-2xl font-medium pointer-events-none">
          +{overflow}
        </div>
      )}
      {editable && onRemove && (
        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation();
            onRemove(attachment);
          }}
          aria-label={`Remove ${attachment.filename}`}
          title="Remove image"
          className="absolute top-1 right-1 opacity-0 group-hover/att:opacity-100 focus:opacity-100 p-1 rounded-full bg-black/60 text-white hover:bg-black/80"
        >
          <X size={14} aria-hidden />
        </button>
      )}
    </figure>
  );
}

/** Build the relative path the keepr-resource:// protocol expects:
 *  `<id>.<ext>` where ext is derived from the MIME's known list. */
function srcForAttachment(a: Attachment): string {
  const ext = mimeToExt(a.mime);
  return `${a.id}.${ext}`;
}

/** NF-V0.5-B — companion thumbnail path. Always `.thumb.jpg` regardless
 *  of source format; the Rust add_image_attachment writes JPEG for the
 *  smallest size. */
function thumbFilename(a: Attachment): string {
  return `${a.id}.thumb.jpg`;
}

function mimeToExt(mime: string): string {
  switch (mime) {
    case "image/png":
      return "png";
    case "image/jpeg":
      return "jpg";
    case "image/gif":
      return "gif";
    case "image/webp":
      return "webp";
    case "image/svg+xml":
      return "svg";
    case "audio/webm":
      return "webm";
    case "audio/ogg":
      return "ogg";
    case "audio/mp4":
      return "m4a";
    case "audio/mpeg":
      return "mp3";
    case "audio/wav":
      return "wav";
    default:
      return "bin";
  }
}
