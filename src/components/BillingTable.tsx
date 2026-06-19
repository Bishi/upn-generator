import type { ComponentPropsWithoutRef, KeyboardEvent, PointerEvent, ReactNode } from "react";
import { useCallback, useLayoutEffect, useRef, useState } from "react";
import { Loader2 } from "lucide-react";
import { cn } from "@/lib/utils";

export const billingTableCellClass = "whitespace-nowrap px-3 py-2.5 align-middle";
export const billingTableTallCellClass = billingTableCellClass;
export const billingTableNumericCellClass = `${billingTableCellClass} text-right font-mono`;
export const billingTableBodyRowClass = "h-[60px] border-b border-border transition-colors hover:bg-accent/10";
export const billingTableZebraRowClass =
  `${billingTableBodyRowClass} odd:bg-card even:bg-surface-2/50`;

type BillingTableFrameProps = ComponentPropsWithoutRef<"div"> & {
  scrollX?: boolean;
  minHeight?: boolean;
};

type ScrollMetrics = {
  hasOverflow: boolean;
  maxScrollLeft: number;
  scrollLeft: number;
  thumbLeftPercent: number;
  thumbWidthPercent: number;
};

const emptyScrollMetrics: ScrollMetrics = {
  hasOverflow: false,
  maxScrollLeft: 0,
  scrollLeft: 0,
  thumbLeftPercent: 0,
  thumbWidthPercent: 100,
};

export function BillingTableFrame({
  className,
  scrollX = true,
  minHeight = false,
  children,
  ...props
}: BillingTableFrameProps) {
  const scrollRef = useRef<HTMLDivElement | null>(null);
  const trackRef = useRef<HTMLDivElement | null>(null);
  const dragRef = useRef<{
    maxScrollLeft: number;
    pointerId: number;
    startScrollLeft: number;
    startX: number;
    thumbWidth: number;
    trackWidth: number;
  } | null>(null);
  const [scrollMetrics, setScrollMetrics] = useState<ScrollMetrics>(emptyScrollMetrics);

  const updateScrollMetrics = useCallback(() => {
    const viewport = scrollRef.current;
    if (!viewport || !scrollX) {
      setScrollMetrics(emptyScrollMetrics);
      return;
    }

    const { clientWidth, scrollLeft, scrollWidth } = viewport;
    const maxScrollLeft = Math.max(0, scrollWidth - clientWidth);
    const hasOverflow = maxScrollLeft > 1;
    const thumbWidthPercent = hasOverflow
      ? Math.max(12, (clientWidth / scrollWidth) * 100)
      : 100;
    const thumbLeftPercent =
      hasOverflow && maxScrollLeft > 0
        ? (scrollLeft / maxScrollLeft) * (100 - thumbWidthPercent)
        : 0;

    setScrollMetrics({
      hasOverflow,
      maxScrollLeft,
      scrollLeft,
      thumbLeftPercent,
      thumbWidthPercent,
    });
  }, [scrollX]);

  useLayoutEffect(() => {
    if (!scrollX) {
      setScrollMetrics(emptyScrollMetrics);
      return;
    }

    const viewport = scrollRef.current;
    if (!viewport) return;

    updateScrollMetrics();
    const resizeObserver = new ResizeObserver(updateScrollMetrics);
    resizeObserver.observe(viewport);
    if (viewport.firstElementChild) resizeObserver.observe(viewport.firstElementChild);
    window.addEventListener("resize", updateScrollMetrics);

    return () => {
      resizeObserver.disconnect();
      window.removeEventListener("resize", updateScrollMetrics);
    };
  }, [scrollX, children, updateScrollMetrics]);

  const scrollToThumbPosition = (position: number) => {
    const viewport = scrollRef.current;
    const track = trackRef.current;
    if (!viewport || !track || scrollMetrics.maxScrollLeft <= 0) return;

    const thumbWidth = (scrollMetrics.thumbWidthPercent / 100) * track.clientWidth;
    const maxThumbLeft = Math.max(1, track.clientWidth - thumbWidth);
    viewport.scrollLeft =
      (Math.min(maxThumbLeft, Math.max(0, position)) / maxThumbLeft) *
      scrollMetrics.maxScrollLeft;
  };

  const handleTrackPointerDown = (event: PointerEvent<HTMLDivElement>) => {
    if (event.target !== event.currentTarget) return;

    const track = trackRef.current;
    if (!track) return;

    const rect = track.getBoundingClientRect();
    const thumbWidth = (scrollMetrics.thumbWidthPercent / 100) * track.clientWidth;
    scrollToThumbPosition(event.clientX - rect.left - thumbWidth / 2);
  };

  const handleThumbPointerDown = (event: PointerEvent<HTMLButtonElement>) => {
    const viewport = scrollRef.current;
    const track = trackRef.current;
    if (!viewport || !track || scrollMetrics.maxScrollLeft <= 0) return;

    event.preventDefault();
    event.currentTarget.setPointerCapture(event.pointerId);
    dragRef.current = {
      maxScrollLeft: scrollMetrics.maxScrollLeft,
      pointerId: event.pointerId,
      startScrollLeft: viewport.scrollLeft,
      startX: event.clientX,
      thumbWidth: (scrollMetrics.thumbWidthPercent / 100) * track.clientWidth,
      trackWidth: track.clientWidth,
    };
  };

  const handleThumbPointerMove = (event: PointerEvent<HTMLButtonElement>) => {
    const viewport = scrollRef.current;
    const drag = dragRef.current;
    if (!viewport || !drag || drag.pointerId !== event.pointerId) return;

    const maxThumbTravel = Math.max(1, drag.trackWidth - drag.thumbWidth);
    const nextScrollLeft =
      drag.startScrollLeft +
      ((event.clientX - drag.startX) / maxThumbTravel) * drag.maxScrollLeft;
    viewport.scrollLeft = Math.min(drag.maxScrollLeft, Math.max(0, nextScrollLeft));
  };

  const endThumbDrag = (event: PointerEvent<HTMLButtonElement>) => {
    if (dragRef.current?.pointerId === event.pointerId) {
      dragRef.current = null;
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
  };

  const handleThumbKeyDown = (event: KeyboardEvent<HTMLButtonElement>) => {
    const viewport = scrollRef.current;
    if (!viewport) return;

    const step = Math.max(48, viewport.clientWidth * 0.2);
    if (event.key === "ArrowLeft") {
      event.preventDefault();
      viewport.scrollLeft -= step;
    } else if (event.key === "ArrowRight") {
      event.preventDefault();
      viewport.scrollLeft += step;
    } else if (event.key === "Home") {
      event.preventDefault();
      viewport.scrollLeft = 0;
    } else if (event.key === "End") {
      event.preventDefault();
      viewport.scrollLeft = scrollMetrics.maxScrollLeft;
    }
  };

  return (
    <div
      className={cn(
        minHeight && "min-h-[268px]",
        "overflow-hidden rounded-lg border border-border bg-card shadow-card",
        className,
      )}
      {...props}
    >
      <div
        ref={scrollRef}
        className={cn(
          scrollX && "billing-table-scroll overflow-x-auto",
        )}
        onScroll={updateScrollMetrics}
      >
        {children}
      </div>
      {scrollX && scrollMetrics.hasOverflow && (
        <div className="bg-card px-3 py-1.5">
          <div
            ref={trackRef}
            className="relative h-2 rounded-full bg-surface-3"
            onPointerDown={handleTrackPointerDown}
          >
            <button
              type="button"
              aria-label="Scroll table horizontally"
              className="absolute top-1/2 h-1.5 -translate-y-1/2 rounded-full bg-border-2 transition-colors hover:bg-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              style={{
                left: `${scrollMetrics.thumbLeftPercent}%`,
                width: `${scrollMetrics.thumbWidthPercent}%`,
              }}
              onKeyDown={handleThumbKeyDown}
              onPointerCancel={endThumbDrag}
              onPointerDown={handleThumbPointerDown}
              onPointerMove={handleThumbPointerMove}
              onPointerUp={endThumbDrag}
            />
          </div>
        </div>
      )}
    </div>
  );
}

export function BillingTable({
  className,
  ...props
}: ComponentPropsWithoutRef<"table">) {
  return <table className={cn("w-full min-w-max text-sm", className)} {...props} />;
}

export function BillingTableHeaderRow({
  className,
  ...props
}: ComponentPropsWithoutRef<"tr">) {
  return (
    <tr
      className={cn(
        "border-b border-border bg-surface-2 text-left text-xs font-medium text-muted-foreground",
        className,
      )}
      {...props}
    />
  );
}

export function BillingTableHeaderCell({
  className,
  ...props
}: ComponentPropsWithoutRef<"th">) {
  return <th className={cn("whitespace-nowrap px-3 pt-3.5 pb-2.5", className)} {...props} />;
}

export function BillingTableFooterRow({
  className,
  ...props
}: ComponentPropsWithoutRef<"tr">) {
  return (
    <tr
      className={cn("bg-surface-2 font-semibold", className)}
      {...props}
    />
  );
}

export function SummaryStrip({
  className,
  ...props
}: ComponentPropsWithoutRef<"div">) {
  return (
    <div
      className={cn(
        "flex flex-wrap items-center gap-2 rounded-lg border border-border bg-card px-4 py-3 text-sm shadow-card",
        className,
      )}
      {...props}
    />
  );
}

export function SummaryChip({
  className,
  ...props
}: ComponentPropsWithoutRef<"span">) {
  return (
    <span
      className={cn(
        "inline-flex items-center gap-2 rounded-md px-3 py-1 text-xs font-semibold",
        className,
      )}
      {...props}
    />
  );
}

type BillingEmptyStateProps = {
  loading?: boolean;
  loadingLabel?: string;
  title: string;
  detail: string;
  action?: ReactNode;
};

export function BillingEmptyState({
  loading = false,
  loadingLabel = "Loading...",
  title,
  detail,
  action,
}: BillingEmptyStateProps) {
  if (loading) {
    return (
      <div className="flex min-h-[268px] items-center justify-center px-6 py-8 text-center">
        <div className="flex items-center gap-2 text-sm text-muted-foreground">
          <Loader2 className="size-4 animate-spin" />
          {loadingLabel}
        </div>
      </div>
    );
  }

  return (
    <div className="relative min-h-[268px] px-6 py-8 text-center">
      <div className="absolute inset-x-6 top-1/2 -translate-y-1/2">
        <div className="mx-auto max-w-md space-y-2">
          <div className="text-sm font-medium">{title}</div>
          <div className="min-h-10 text-sm leading-5 text-muted-foreground">{detail}</div>
        </div>
      </div>
      {action && <div className="absolute inset-x-6 bottom-14 flex justify-center">{action}</div>}
    </div>
  );
}
