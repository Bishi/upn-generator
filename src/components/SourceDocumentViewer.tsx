import { useEffect, useState } from "react";
import { Loader2, Minus, Plus, X } from "lucide-react";
import type { SourceDocumentInfo } from "@/lib/types";
import { Button } from "@/components/ui/button";

const TIFF_DECODE_TIMEOUT_MS = 10_000;

type TiffWorkerResponse =
  | { ok: true; width: number; height: number; rgba: ArrayBuffer }
  | { ok: false; error: string };

interface TiffDecodeTask {
  promise: Promise<{ width: number; height: number; rgba: ArrayBuffer }>;
  cancel: () => void;
}

function isTiff(mediaType: string) {
  return mediaType === "image/tiff" || mediaType === "image/tif";
}

function startTiffDecode(bytes: ArrayBuffer): TiffDecodeTask {
  const worker = new Worker(
    new URL("../workers/tiffDecoder.worker.ts", import.meta.url),
    { type: "module" },
  );
  let settled = false;
  let rejectTask: (reason: Error) => void = () => undefined;
  let timeoutId = 0;

  const finish = () => {
    window.clearTimeout(timeoutId);
    worker.terminate();
  };
  const promise = new Promise<{ width: number; height: number; rgba: ArrayBuffer }>((resolve, reject) => {
    rejectTask = reject;
    worker.onmessage = ({ data }: MessageEvent<TiffWorkerResponse>) => {
      if (settled) return;
      settled = true;
      finish();
      if (data.ok) resolve(data);
      else reject(new Error(data.error));
    };
    worker.onerror = () => {
      if (settled) return;
      settled = true;
      finish();
      reject(new Error("The TIFF decoder failed."));
    };
    worker.onmessageerror = () => {
      if (settled) return;
      settled = true;
      finish();
      reject(new Error("The TIFF decoder returned an invalid response."));
    };
    timeoutId = window.setTimeout(() => {
      if (settled) return;
      settled = true;
      finish();
      reject(new Error("The TIFF took too long to decode and was stopped."));
    }, TIFF_DECODE_TIMEOUT_MS);
    try {
      worker.postMessage(bytes, [bytes]);
    } catch (reason) {
      settled = true;
      finish();
      reject(reason instanceof Error ? reason : new Error(String(reason)));
    }
  });

  return {
    promise,
    cancel: () => {
      if (settled) return;
      settled = true;
      finish();
      rejectTask(new Error("TIFF decoding was cancelled."));
    },
  };
}

async function renderTiffToPngUrl({
  width,
  height,
  rgba,
}: Awaited<TiffDecodeTask["promise"]>): Promise<string> {
  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  const context = canvas.getContext("2d");
  if (!context) throw new Error("The TIFF viewer could not initialize.");
  context.putImageData(
    new ImageData(new Uint8ClampedArray(rgba), width, height),
    0,
    0,
  );
  const png = await new Promise<Blob>((resolve, reject) => {
    canvas.toBlob(
      (blob) => blob ? resolve(blob) : reject(new Error("The TIFF could not be rendered.")),
      "image/png",
    );
  });
  return URL.createObjectURL(png);
}

export interface SourceDocumentLoadResult {
  info: SourceDocumentInfo;
  bytes: ArrayBuffer;
}

interface SourceDocumentViewerProps {
  open: boolean;
  load: (() => Promise<SourceDocumentLoadResult>) | null;
  startPage?: number | null;
  layout?: "fullscreen" | "inbox-companion";
  onClose: () => void;
}

export function SourceDocumentViewer({
  open,
  load,
  startPage,
  layout = "fullscreen",
  onClose,
}: SourceDocumentViewerProps) {
  const [info, setInfo] = useState<SourceDocumentInfo | null>(null);
  const [objectUrl, setObjectUrl] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [zoom, setZoom] = useState(1);

  useEffect(() => {
    if (!open || !load) return;
    let cancelled = false;
    let nextUrl: string | null = null;
    let cancelTiffDecode: (() => void) | null = null;
    setInfo(null);
    setObjectUrl(null);
    setError(null);
    setZoom(1);
    void load()
      .then(async ({ info: nextInfo, bytes }) => {
        if (cancelled) return;
        if (nextInfo.media_type !== "application/pdf" && !nextInfo.media_type.startsWith("image/")) {
          throw new Error("This original document type cannot be displayed safely.");
        }
        if (isTiff(nextInfo.media_type)) {
          const task = startTiffDecode(bytes);
          cancelTiffDecode = task.cancel;
          const decoded = await task.promise;
          cancelTiffDecode = null;
          nextUrl = await renderTiffToPngUrl(decoded);
        } else {
          nextUrl = URL.createObjectURL(new Blob([bytes], { type: nextInfo.media_type }));
        }
        if (cancelled) {
          URL.revokeObjectURL(nextUrl);
          nextUrl = null;
          return;
        }
        setInfo(nextInfo);
        setObjectUrl(nextUrl);
      })
      .catch((reason) => {
        if (!cancelled) setError(String(reason));
      });
    return () => {
      cancelled = true;
      cancelTiffDecode?.();
      if (nextUrl) URL.revokeObjectURL(nextUrl);
    };
  }, [load, open]);

  useEffect(() => {
    if (!open) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onClose, open]);

  if (!open) return null;
  const failLoad = (message: string) => {
    if (objectUrl) URL.revokeObjectURL(objectUrl);
    setObjectUrl(null);
    setError(message);
  };
  const page = startPage ?? info?.page_start;
  const viewerUrl = objectUrl && info?.media_type === "application/pdf" && page
    ? `${objectUrl}#page=${page}`
    : objectUrl;

  return (
    <div
      className={layout === "inbox-companion"
        ? "fixed inset-0 z-[60] flex flex-col bg-background/95 backdrop-blur-sm xl:right-[760px] xl:border-r xl:border-border xl:shadow-pop"
        : "fixed inset-0 z-[140] flex flex-col bg-background/95 backdrop-blur-sm"}
    >
      <div className="flex min-h-16 items-center gap-3 border-b border-border bg-card px-4 py-3">
        <div className="min-w-0 flex-1">
          <h2 className="truncate font-head text-lg font-semibold">{info?.original_name ?? "Original document"}</h2>
          <p className="text-xs text-muted-foreground">
            {info ? `${(info.byte_size / 1024 / 1024).toFixed(2)} MB` : "Loading exact retained source…"}
          </p>
        </div>
        {info?.media_type.startsWith("image/") && (
          <div className="flex items-center gap-1">
            <Button variant="outline" size="icon" onClick={() => setZoom((value) => Math.max(0.25, value - 0.25))} aria-label="Zoom out">
              <Minus className="size-4" />
            </Button>
            <span className="w-14 text-center text-xs font-semibold">{Math.round(zoom * 100)}%</span>
            <Button variant="outline" size="icon" onClick={() => setZoom((value) => Math.min(4, value + 0.25))} aria-label="Zoom in">
              <Plus className="size-4" />
            </Button>
          </div>
        )}
        <Button variant="ghost" size="icon" onClick={onClose} aria-label="Close original document">
          <X className="size-5" />
        </Button>
      </div>
      <div className="min-h-0 flex-1 overflow-auto bg-surface-2 p-3">
        {!objectUrl && !error && (
          <div className="grid h-full place-items-center text-sm text-muted-foreground">
            <Loader2 className="size-6 animate-spin" />
          </div>
        )}
        {error && (
          <div role="alert" className="mx-auto mt-12 max-w-xl rounded-lg border border-danger/30 bg-danger-soft p-5 text-sm text-danger">
            Original unavailable: {error}
          </div>
        )}
        {viewerUrl && info?.media_type === "application/pdf" && (
          <iframe
            key={viewerUrl}
            title={info.original_name}
            src={viewerUrl}
            className="h-full min-h-[500px] w-full rounded-md bg-white"
            onError={() => failLoad("The PDF viewer could not load this document.")}
          />
        )}
        {objectUrl && info?.media_type.startsWith("image/") && (
          <div
            className="flex min-h-full items-start overflow-auto"
            style={{ justifyContent: "safe center" }}
          >
            <img
              src={objectUrl}
              alt={info.original_name}
              style={{ width: `${zoom * 100}%` }}
              className="h-auto max-w-none shrink-0 rounded-md bg-white shadow"
              onError={() => failLoad("The image viewer could not load this document.")}
            />
          </div>
        )}
      </div>
    </div>
  );
}
