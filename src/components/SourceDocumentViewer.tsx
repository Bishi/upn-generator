import { useEffect, useState } from "react";
import { Loader2, Minus, Plus, X } from "lucide-react";
import type { SourceDocumentInfo } from "@/lib/types";
import { Button } from "@/components/ui/button";

export interface SourceDocumentLoadResult {
  info: SourceDocumentInfo;
  bytes: ArrayBuffer;
}

interface SourceDocumentViewerProps {
  open: boolean;
  load: (() => Promise<SourceDocumentLoadResult>) | null;
  startPage?: number | null;
  onClose: () => void;
}

export function SourceDocumentViewer({
  open,
  load,
  startPage,
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
    setInfo(null);
    setObjectUrl(null);
    setError(null);
    setZoom(1);
    void load()
      .then(({ info: nextInfo, bytes }) => {
        if (cancelled) return;
        if (nextInfo.media_type !== "application/pdf" && !nextInfo.media_type.startsWith("image/")) {
          throw new Error("This original document type cannot be displayed safely.");
        }
        nextUrl = URL.createObjectURL(new Blob([bytes], { type: nextInfo.media_type }));
        setInfo(nextInfo);
        setObjectUrl(nextUrl);
      })
      .catch((reason) => {
        if (!cancelled) setError(String(reason));
      });
    return () => {
      cancelled = true;
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
    <div className="fixed inset-0 z-[140] flex flex-col bg-background/95 backdrop-blur-sm">
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
          <div className="flex min-h-full items-start justify-center overflow-auto">
            <img
              src={objectUrl}
              alt={info.original_name}
              style={{ width: `${zoom * 100}%` }}
              className="h-auto max-w-none rounded-md bg-white shadow"
              onError={() => failLoad("The image viewer could not load this document.")}
            />
          </div>
        )}
      </div>
    </div>
  );
}
