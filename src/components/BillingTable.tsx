import type { ComponentPropsWithoutRef, ReactNode } from "react";
import { Loader2 } from "lucide-react";
import { cn } from "@/lib/utils";

export const billingTableCellClass = "px-3 py-2";
export const billingTableTallCellClass = "px-3 py-3";
export const billingTableNumericCellClass = `${billingTableCellClass} text-right font-mono`;
export const billingTableBodyRowClass = "border-b border-border transition-colors hover:bg-accent/10";
export const billingTableZebraRowClass =
  "border-b border-border odd:bg-card even:bg-surface-2/50 transition-colors hover:bg-accent/10";

type BillingTableFrameProps = ComponentPropsWithoutRef<"div"> & {
  scrollX?: boolean;
  minHeight?: boolean;
};

export function BillingTableFrame({
  className,
  scrollX = false,
  minHeight = false,
  children,
  ...props
}: BillingTableFrameProps) {
  return (
    <div
      className={cn(
        minHeight && "min-h-[268px]",
        scrollX ? "overflow-x-auto" : "overflow-hidden",
        "rounded-lg border border-border bg-card shadow-card",
        className,
      )}
      {...props}
    >
      {children}
    </div>
  );
}

export function BillingTable({
  className,
  ...props
}: ComponentPropsWithoutRef<"table">) {
  return <table className={cn("w-full text-sm", className)} {...props} />;
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
  return <th className={cn("px-3 pt-3.5 pb-2.5", className)} {...props} />;
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
