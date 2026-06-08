import { type ReactNode } from "react";

type BillingPageShellProps = {
  title: string;
  subtitle?: string | null;
  actions?: ReactNode;
  children: ReactNode;
};

export function BillingPageShell({
  title,
  subtitle,
  actions,
  children,
}: BillingPageShellProps) {
  const hasSubtitle = Boolean(subtitle?.trim());

  return (
    <div className="flex flex-col gap-6">
      <section className="grid gap-4">
        <div className="flex min-h-[58px] flex-wrap items-start justify-between gap-4">
          <div className="min-h-[58px]">
            <h2 className="text-3xl font-semibold leading-tight">{title}</h2>
            <p className="mt-1 min-h-5 text-sm text-muted-foreground">
              {hasSubtitle ? subtitle : "\u00A0"}
            </p>
          </div>
          <div className="flex min-h-[58px] flex-wrap items-start justify-end gap-2">
            {actions ?? <div className="h-9" />}
          </div>
        </div>
      </section>

      {children}
    </div>
  );
}
