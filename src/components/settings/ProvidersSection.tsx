import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useMemo, useState, type FormEvent, type ReactNode } from "react";
import { Loader2, Plus, Save, Trash2, X } from "lucide-react";
import { ipc } from "@/lib/ipc";
import { serviceIconFor } from "@/lib/service-icons";
import { useWorkflowSnapshotContext } from "@/lib/workflow-snapshot";
import type { Provider } from "@/lib/types";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { SettingsLoadingCard } from "@/components/settings/SettingsLoadingCard";
import { cn } from "@/lib/utils";

const PURPOSE_CODES = ["OTHR", "ENRG", "WTER", "SCVE", "SALA", "RENT", "COST"];

const newProvider = (): Provider => ({
  id: null,
  name: "",
  service_type: "",
  creditor_name: "",
  creditor_address: "",
  creditor_city: "",
  creditor_postal_code: "",
  creditor_iban: "",
  purpose_code: "OTHR",
  match_pattern: "",
  amount_pattern: "",
  reference_pattern: "",
  due_date_pattern: "",
  invoice_number_pattern: "",
  purpose_text_template: "",
  split_basis: "m2_percentage",
});

function splitBasisLabel(splitBasis: Provider["split_basis"]) {
  switch (splitBasis) {
    case "occupants":
      return "people";
    case "equal_apartments":
      return "equal";
    default:
      return "m2";
  }
}

function splitBasisLongLabel(splitBasis: Provider["split_basis"]) {
  switch (splitBasis) {
    case "occupants":
      return "People (occupants)";
    case "equal_apartments":
      return "Equal (all apartments)";
    default:
      return "m2 (surface area)";
  }
}

function cloneProvider(provider: Provider) {
  return { ...provider };
}

function providersEqual(a: Provider | null, b: Provider | null) {
  if (!a || !b) return a === b;

  return (
    a.id === b.id &&
    a.name === b.name &&
    a.service_type === b.service_type &&
    a.creditor_name === b.creditor_name &&
    a.creditor_address === b.creditor_address &&
    a.creditor_city === b.creditor_city &&
    a.creditor_postal_code === b.creditor_postal_code &&
    a.creditor_iban === b.creditor_iban &&
    a.purpose_code === b.purpose_code &&
    a.match_pattern === b.match_pattern &&
    a.amount_pattern === b.amount_pattern &&
    a.reference_pattern === b.reference_pattern &&
    a.due_date_pattern === b.due_date_pattern &&
    a.invoice_number_pattern === b.invoice_number_pattern &&
    a.purpose_text_template === b.purpose_text_template &&
    a.split_basis === b.split_basis
  );
}

export function ProvidersSection() {
  const queryClient = useQueryClient();
  const snapshot = useWorkflowSnapshotContext();
  const { data: providers = [], isLoading } = useQuery({
    queryKey: ["providers"],
    queryFn: ipc.getProviders,
    initialData: snapshot.providers.length > 0 ? snapshot.providers : undefined,
  });

  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [editing, setEditing] = useState<Provider | null>(null);
  const [baseline, setBaseline] = useState<Provider | null>(null);
  const [isNew, setIsNew] = useState(false);

  const splitBasisCounts = providers.reduce(
    (counts, provider) => {
      counts[provider.split_basis] += 1;
      return counts;
    },
    { occupants: 0, m2_percentage: 0, equal_apartments: 0 } as Record<
      Provider["split_basis"],
      number
    >,
  );

  const selectedProvider = useMemo(
    () => providers.find((provider) => provider.id === selectedId) ?? providers[0] ?? null,
    [providers, selectedId],
  );

  useEffect(() => {
    if (isNew) return;
    if (selectedProvider) {
      const draft = cloneProvider(selectedProvider);
      setSelectedId(selectedProvider.id);
      setEditing(draft);
      setBaseline(cloneProvider(selectedProvider));
    } else {
      setSelectedId(null);
      setEditing(null);
      setBaseline(null);
    }
  }, [isNew, selectedProvider]);

  const saveMutation = useMutation({
    mutationFn: ipc.saveProvider,
    onSuccess: async (savedProvider) => {
      await queryClient.invalidateQueries({ queryKey: ["providers"] });
      await snapshot.refresh({ core: true, periods: false, selected: true, statuses: true });
      setSelectedId(savedProvider.id);
      const draft = cloneProvider(savedProvider);
      setEditing(draft);
      setBaseline(cloneProvider(savedProvider));
      setIsNew(false);
    },
  });

  const deleteMutation = useMutation({
    mutationFn: ipc.deleteProvider,
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["providers"] });
      await snapshot.refresh({ core: true, periods: false, selected: true, statuses: true });
      setSelectedId(null);
      setEditing(null);
      setBaseline(null);
      setIsNew(false);
    },
  });

  const handleSelect = (provider: Provider) => {
    const draft = cloneProvider(provider);
    setSelectedId(provider.id);
    setEditing(draft);
    setBaseline(cloneProvider(provider));
    setIsNew(false);
  };

  const handleNew = () => {
    const draft = newProvider();
    setSelectedId(null);
    setEditing(draft);
    setBaseline(cloneProvider(draft));
    setIsNew(true);
  };

  const handleSave = (event: FormEvent) => {
    event.preventDefault();
    if (editing && !providersEqual(editing, baseline)) saveMutation.mutate(editing);
  };

  const handleDiscard = () => {
    if (isNew) {
      setIsNew(false);
      if (selectedProvider) {
        const draft = cloneProvider(selectedProvider);
        setEditing(draft);
        setBaseline(cloneProvider(selectedProvider));
      } else {
        setEditing(null);
        setBaseline(null);
      }
      return;
    }
    if (baseline) setEditing(cloneProvider(baseline));
  };

  const isDirty = !providersEqual(editing, baseline);

  if (isLoading) return <SettingsLoadingCard className="max-w-none" rows={3} />;

  return (
    <div className="space-y-4">
      <Card className="p-3">
        <div className="flex flex-wrap items-center gap-2">
          <span className="px-1 text-[11px] font-bold uppercase tracking-wide text-muted-foreground">
            Providers
          </span>
          <Badge variant="secondary" className="gap-1.5">
            {providers.length} configured
          </Badge>
          <Badge variant="secondary" className="gap-1.5">
            {splitBasisCounts.m2_percentage} m2
          </Badge>
          <Badge variant="secondary" className="gap-1.5">
            {splitBasisCounts.occupants} people
          </Badge>
          <Badge variant="secondary" className="gap-1.5">
            {splitBasisCounts.equal_apartments} equal
          </Badge>
          <Button onClick={handleNew} size="sm" className="ml-auto gap-2">
            <Plus className="size-4" />
            Add Provider
          </Button>
        </div>
      </Card>

      <div className="grid gap-4 lg:grid-cols-[300px_minmax(0,1fr)]">
        <Card className="overflow-hidden">
          <div className="flex items-center justify-between border-b border-border px-4 py-3">
            <div className="text-sm font-semibold">Providers</div>
          </div>

          <div className="divide-y divide-border">
            {providers.map((provider) => {
              const isSelected = !isNew && provider.id === editing?.id;
              const ProviderIcon = serviceIconFor(provider.service_type, provider.name);
              return (
                <button
                  type="button"
                  key={provider.id}
                  onClick={() => handleSelect(provider)}
                  className={cn(
                    "flex w-full gap-3 px-4 py-3 text-left transition-colors hover:bg-accent/40",
                    isSelected && "bg-accent-soft text-accent-foreground",
                  )}
                >
                  <span className="mt-0.5 grid size-8 shrink-0 place-items-center rounded-md bg-surface-3 text-muted-foreground">
                    <ProviderIcon className="size-4" />
                  </span>
                  <span className="min-w-0 flex-1">
                    <span className="block truncate text-sm font-semibold">
                      {provider.name || "Unnamed provider"}
                    </span>
                    <span className="mt-1 flex flex-wrap gap-1.5">
                      <Badge variant="outline" className="h-5 px-2 text-[10px]">
                        {provider.purpose_code || "OTHR"}
                      </Badge>
                      <Badge variant="secondary" className="h-5 px-2 text-[10px]">
                        {splitBasisLabel(provider.split_basis)}
                      </Badge>
                    </span>
                    <span className="mt-1 block truncate font-mono text-xs text-muted-foreground">
                      {provider.creditor_iban || "No IBAN set"}
                    </span>
                  </span>
                </button>
              );
            })}
          </div>

          {providers.length === 0 && (
            <div className="px-4 py-8 text-center text-sm text-muted-foreground">
              No providers yet. Add your first provider to get started.
            </div>
          )}
        </Card>

        <ProviderDetail
          provider={editing}
          isNew={isNew}
          isDirty={isDirty}
          isSaving={saveMutation.isPending}
          isDeleting={deleteMutation.isPending}
          onChange={setEditing}
          onSave={handleSave}
          onDiscard={handleDiscard}
          onDelete={() => editing?.id && deleteMutation.mutate(editing.id)}
        />
      </div>
    </div>
  );
}

function ProviderDetail({
  provider,
  isNew,
  isDirty,
  isSaving,
  isDeleting,
  onChange,
  onSave,
  onDiscard,
  onDelete,
}: {
  provider: Provider | null;
  isNew: boolean;
  isDirty: boolean;
  isSaving: boolean;
  isDeleting: boolean;
  onChange: (provider: Provider) => void;
  onSave: (event: FormEvent) => void;
  onDiscard: () => void;
  onDelete: () => void;
}) {
  if (!provider) {
    return (
      <Card className="flex min-h-72 items-center justify-center p-8 text-center text-sm text-muted-foreground">
        Select a provider to edit creditor details and parser patterns.
      </Card>
    );
  }

  const regexFields: Array<{ key: keyof Provider; label: string; placeholder: string }> = [
    { key: "match_pattern", label: "Match pattern", placeholder: "Identifies this provider" },
    { key: "amount_pattern", label: "Amount pattern", placeholder: "ZA PLACILO.*?([\\d,.]+)" },
    { key: "reference_pattern", label: "Reference pattern", placeholder: "SI\\d{2}\\s+[\\d\\s]+" },
    { key: "due_date_pattern", label: "Due date pattern", placeholder: "Rok placila:\\s*([\\d.]+)" },
    { key: "invoice_number_pattern", label: "Invoice number", placeholder: "Racun.*?([A-Z0-9-]+)" },
    { key: "purpose_text_template", label: "Purpose template", placeholder: "rn. {invoice} ({month}-{year})" },
  ];

  return (
    <Card className="overflow-hidden">
      <div className="border-b border-border px-5 py-3">
        <div>
          <h3 className="font-head text-lg font-semibold">
            {isNew ? "New provider" : provider.name || "Unnamed provider"}
          </h3>
        </div>
      </div>

      <form onSubmit={onSave}>
        <CardContent className="grid gap-6 p-5 xl:grid-cols-2">
          <div className="space-y-4 xl:border-r xl:border-border xl:pr-6">
            <div className="grid grid-cols-2 gap-3">
              <Field label="Provider name">
                <Input
                  value={provider.name}
                  onChange={(event) => onChange({ ...provider, name: event.target.value })}
                  required
                />
              </Field>
              <Field label="Service type">
                <Input
                  value={provider.service_type}
                  onChange={(event) =>
                    onChange({ ...provider, service_type: event.target.value })
                  }
                />
              </Field>
            </div>

            <Field label="Split basis">
              <select
                value={provider.split_basis}
                onChange={(event) =>
                  onChange({
                    ...provider,
                    split_basis: event.target.value as Provider["split_basis"],
                  })
                }
                className="flex h-9 w-full rounded-md border border-input bg-card px-3 py-1 text-sm shadow-card focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
              >
                <option value="m2_percentage">{splitBasisLongLabel("m2_percentage")}</option>
                <option value="occupants">{splitBasisLongLabel("occupants")}</option>
                <option value="equal_apartments">
                  {splitBasisLongLabel("equal_apartments")}
                </option>
              </select>
            </Field>

            <div className="border-t border-border pt-4 text-[11px] font-bold uppercase tracking-wide text-muted-foreground">
              Creditor details
            </div>
            <Field label="Creditor name">
              <Input
                value={provider.creditor_name}
                onChange={(event) =>
                  onChange({ ...provider, creditor_name: event.target.value })
                }
              />
            </Field>
            <Field label="Creditor address">
              <Input
                value={provider.creditor_address}
                onChange={(event) =>
                  onChange({ ...provider, creditor_address: event.target.value })
                }
              />
            </Field>
            <div className="grid grid-cols-[0.8fr_1.2fr] gap-3">
              <Field label="Postal code">
                <Input
                  value={provider.creditor_postal_code}
                  onChange={(event) =>
                    onChange({ ...provider, creditor_postal_code: event.target.value })
                  }
                />
              </Field>
              <Field label="City">
                <Input
                  value={provider.creditor_city}
                  onChange={(event) =>
                    onChange({ ...provider, creditor_city: event.target.value })
                  }
                />
              </Field>
            </div>
            <div className="grid grid-cols-[1.4fr_0.8fr] gap-3">
              <Field label="IBAN">
                <Input
                  value={provider.creditor_iban}
                  onChange={(event) =>
                    onChange({ ...provider, creditor_iban: event.target.value })
                  }
                  className="font-mono"
                />
              </Field>
              <Field label="Purpose code">
                <select
                  value={provider.purpose_code}
                  onChange={(event) =>
                    onChange({ ...provider, purpose_code: event.target.value })
                  }
                  className="flex h-9 w-full rounded-md border border-input bg-card px-3 py-1 font-mono text-sm shadow-card focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
                >
                  {PURPOSE_CODES.map((code) => (
                    <option key={code} value={code}>
                      {code}
                    </option>
                  ))}
                </select>
              </Field>
            </div>
          </div>

          <div className="space-y-3">
            <div>
              <div className="text-[11px] font-bold uppercase tracking-wide text-muted-foreground">
                Parsing patterns (regex)
              </div>
              <p className="mt-1 text-xs text-muted-foreground">
                Capture group 1 is used as the extracted value.
              </p>
            </div>
            {regexFields.map(({ key, label, placeholder }) => (
              <Field key={key} label={label}>
                <Input
                  value={String(provider[key] ?? "")}
                  onChange={(event) => onChange({ ...provider, [key]: event.target.value })}
                  placeholder={placeholder}
                  className="font-mono text-xs"
                />
              </Field>
            ))}
          </div>
        </CardContent>

        <div className="flex flex-wrap items-center gap-2 border-t border-border px-5 py-4">
          <Button type="submit" disabled={!isDirty || isSaving || isDeleting} className="gap-2">
            {isSaving ? (
              <Loader2 className="size-4 animate-spin" />
            ) : (
              <Save className="size-4" />
            )}
            {isNew ? "Save provider" : "Save changes"}
          </Button>
          <Button
            type="button"
            variant="outline"
            disabled={(!isNew && !isDirty) || isSaving || isDeleting}
            onClick={onDiscard}
            className="gap-2"
          >
            <X className="size-4" />
            Discard
          </Button>
          {!isNew && (
            <Button
              type="button"
              variant="outline"
              className="ml-auto gap-2 text-danger hover:text-danger"
              disabled={isDeleting}
              onClick={onDelete}
            >
              {isDeleting ? (
                <Loader2 className="size-4 animate-spin" />
              ) : (
                <Trash2 className="size-4" />
              )}
              Delete
            </Button>
          )}
        </div>
      </form>
    </Card>
  );
}

function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="space-y-1.5">
      <Label>{label}</Label>
      {children}
    </div>
  );
}
