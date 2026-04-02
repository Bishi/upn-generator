import { useState, type ReactNode } from "react";
import { Check, X, CalendarPlus } from "lucide-react";
import type { BillingPeriod } from "@/lib/types";
import { MONTHS } from "@/lib/types";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

type BillingPageShellProps = {
  title: string;
  subtitle?: string | null;
  years: number[];
  selectedYear: number;
  onSelectYear: (year: number) => void;
  yearPeriods: BillingPeriod[];
  selected: BillingPeriod | null;
  onSelectPeriod: (period: BillingPeriod) => void;
  onAddYear?: (year: number) => void | Promise<void>;
  actions?: ReactNode;
  children: ReactNode;
};

export function BillingPageShell({
  title,
  subtitle,
  years,
  selectedYear,
  onSelectYear,
  yearPeriods,
  selected,
  onSelectPeriod,
  onAddYear,
  actions,
  children,
}: BillingPageShellProps) {
  const [showAddYear, setShowAddYear] = useState(false);
  const [newYear, setNewYear] = useState(new Date().getFullYear());

  const hasSubtitle = Boolean(subtitle?.trim());

  return (
    <div className="flex flex-col gap-6">
      <section className="grid gap-4">
        <div className="flex min-h-[72px] flex-wrap items-start justify-between gap-4">
          <div className="min-h-[72px]">
            <h2 className="text-2xl font-bold">{title}</h2>
            <p className="mt-1 min-h-5 text-sm text-muted-foreground">
              {hasSubtitle ? subtitle : "\u00A0"}
            </p>
          </div>
          <div className="flex min-h-[72px] flex-wrap items-start justify-end gap-2">
            {actions ?? <div className="h-9" />}
          </div>
        </div>

        <div className="grid gap-3">
          <div className="min-h-9 flex items-center">
            <div className="flex flex-wrap items-center gap-2">
              {years.map((year) => (
                <button
                  key={year}
                  onClick={() => onSelectYear(year)}
                  className={`px-3 py-1.5 rounded-md text-sm font-medium border transition-colors ${
                    selectedYear === year
                      ? "bg-primary text-primary-foreground border-primary"
                      : "border-border hover:bg-accent"
                  }`}
                >
                  {year}
                </button>
              ))}

              {onAddYear ? (
                showAddYear ? (
                  <div className="flex items-center gap-2 rounded-md border bg-card px-3 py-1.5">
                    <Input
                      type="number"
                      className="h-6 w-20 text-sm"
                      value={newYear}
                      onChange={(e) => setNewYear(Number(e.target.value))}
                    />
                    <button
                      className="text-green-600 hover:text-green-700"
                      onClick={() => {
                        void Promise.resolve(onAddYear(newYear)).then(() => {
                          setShowAddYear(false);
                        });
                      }}
                    >
                      <Check className="size-4" />
                    </button>
                    <button
                      className="text-muted-foreground hover:text-foreground"
                      onClick={() => setShowAddYear(false)}
                    >
                      <X className="size-4" />
                    </button>
                  </div>
                ) : (
                  <Button
                    size="sm"
                    variant="outline"
                    onClick={() => setShowAddYear(true)}
                    className="gap-1"
                  >
                    <CalendarPlus className="size-3.5" /> Add Year
                  </Button>
                )
              ) : null}
            </div>
          </div>

          <div className="min-h-9 flex items-center">
            {yearPeriods.length > 0 ? (
              <div className="flex flex-wrap items-center gap-2">
                {yearPeriods.map((period) => (
                  <button
                    key={period.id}
                    onClick={() => onSelectPeriod(period)}
                    className={`px-3 py-1.5 rounded-md text-sm border transition-colors ${
                      selected?.id === period.id
                        ? "bg-primary text-primary-foreground border-primary"
                        : "border-border hover:bg-accent"
                    }`}
                  >
                    {MONTHS[period.month - 1]}
                  </button>
                ))}
              </div>
            ) : (
              <div />
            )}
          </div>
        </div>
      </section>

      {children}
    </div>
  );
}
