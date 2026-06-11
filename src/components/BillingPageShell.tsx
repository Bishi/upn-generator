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
        <div className="flex flex-wrap items-center justify-between gap-4">
          <div>
            <h2 className="text-3xl font-semibold leading-tight">{title}</h2>
            {hasSubtitle && (
              <p className="mt-1 text-sm text-muted-foreground">{subtitle}</p>
            )}
          </div>
          {actions && (
            <div className="flex flex-wrap items-center justify-end gap-2">
              {actions}
            </div>
          )}
        </div>
      </section>

      {children}
    </div>
  );
}
