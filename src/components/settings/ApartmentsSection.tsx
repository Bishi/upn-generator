import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useMemo, useState, type FormEvent, type ReactNode } from "react";
import { Check, Loader2, Plus, Save, Trash2, X } from "lucide-react";
import { ipc } from "@/lib/ipc";
import { useWorkflowSnapshotContext } from "@/lib/workflow-snapshot";
import type { Apartment } from "@/lib/types";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { SettingsLoadingCard } from "@/components/settings/SettingsLoadingCard";
import { cn } from "@/lib/utils";

const newApartment = (): Apartment => ({
  id: null,
  building_id: 1,
  label: "",
  unit_code: "",
  occupant_count: 1,
  contact_email: "",
  payer_name: "",
  payer_address: "",
  payer_city: "Ljubljana",
  payer_postal_code: "1000",
  m2_percentage: 0,
  is_active: true,
});

function cloneApartment(apartment: Apartment) {
  return { ...apartment };
}

function apartmentsEqual(a: Apartment | null, b: Apartment | null) {
  if (!a || !b) return a === b;

  return (
    a.id === b.id &&
    a.building_id === b.building_id &&
    a.label === b.label &&
    a.unit_code === b.unit_code &&
    a.occupant_count === b.occupant_count &&
    a.contact_email === b.contact_email &&
    a.payer_name === b.payer_name &&
    a.payer_address === b.payer_address &&
    a.payer_city === b.payer_city &&
    a.payer_postal_code === b.payer_postal_code &&
    a.m2_percentage === b.m2_percentage &&
    a.is_active === b.is_active
  );
}

export function ApartmentsSection() {
  const queryClient = useQueryClient();
  const snapshot = useWorkflowSnapshotContext();
  const { data: apartments = [], isLoading } = useQuery({
    queryKey: ["apartments"],
    queryFn: ipc.getApartments,
    initialData: snapshot.apartments.length > 0 ? snapshot.apartments : undefined,
  });
  const { data: building } = useQuery({
    queryKey: ["building"],
    queryFn: ipc.getBuilding,
  });

  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [editing, setEditing] = useState<Apartment | null>(null);
  const [baseline, setBaseline] = useState<Apartment | null>(null);
  const [isNew, setIsNew] = useState(false);

  const activeApartments = apartments.filter((apartment) => apartment.is_active);
  const totalOccupants = activeApartments.reduce(
    (sum, apartment) => sum + apartment.occupant_count,
    0,
  );
  const totalM2Percentage = activeApartments.reduce(
    (sum, apartment) => sum + apartment.m2_percentage,
    0,
  );
  const allocationComplete = Math.abs(totalM2Percentage - 100) < 0.01;

  const selectedApartment = useMemo(
    () => apartments.find((apartment) => apartment.id === selectedId) ?? apartments[0] ?? null,
    [apartments, selectedId],
  );

  useEffect(() => {
    if (isNew) return;
    if (selectedApartment) {
      const draft = cloneApartment(selectedApartment);
      setSelectedId(selectedApartment.id);
      setEditing(draft);
      setBaseline(cloneApartment(selectedApartment));
    } else {
      setSelectedId(null);
      setEditing(null);
      setBaseline(null);
    }
  }, [isNew, selectedApartment]);

  const saveMutation = useMutation({
    mutationFn: ipc.saveApartment,
    onSuccess: async (savedApartment) => {
      await queryClient.invalidateQueries({ queryKey: ["apartments"] });
      await snapshot.refresh({ core: true, periods: false, selected: true, statuses: true });
      setSelectedId(savedApartment.id);
      const draft = cloneApartment(savedApartment);
      setEditing(draft);
      setBaseline(cloneApartment(savedApartment));
      setIsNew(false);
    },
  });

  const deleteMutation = useMutation({
    mutationFn: ipc.deleteApartment,
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["apartments"] });
      await snapshot.refresh({ core: true, periods: false, selected: true, statuses: true });
      setSelectedId(null);
      setEditing(null);
      setBaseline(null);
      setIsNew(false);
    },
  });

  const handleSelect = (apartment: Apartment) => {
    const draft = cloneApartment(apartment);
    setSelectedId(apartment.id);
    setEditing(draft);
    setBaseline(cloneApartment(apartment));
    setIsNew(false);
  };

  const handleNew = () => {
    const draft = {
      ...newApartment(),
      payer_address: building?.address ?? "",
      payer_city: building?.city ?? "Ljubljana",
      payer_postal_code: building?.postal_code ?? "1000",
    };
    setSelectedId(null);
    setEditing(draft);
    setBaseline(cloneApartment(draft));
    setIsNew(true);
  };

  const handleSave = (event: FormEvent) => {
    event.preventDefault();
    if (editing && !apartmentsEqual(editing, baseline)) saveMutation.mutate(editing);
  };

  const handleDiscard = () => {
    if (isNew) {
      setIsNew(false);
      if (selectedApartment) {
        const draft = cloneApartment(selectedApartment);
        setEditing(draft);
        setBaseline(cloneApartment(selectedApartment));
      } else {
        setEditing(null);
        setBaseline(null);
      }
      return;
    }
    if (baseline) setEditing(cloneApartment(baseline));
  };

  const isDirty = !apartmentsEqual(editing, baseline);

  if (isLoading) return <SettingsLoadingCard className="max-w-none" rows={3} />;

  return (
    <div className="space-y-4">
      <Card className="p-3">
        <div className="flex flex-wrap items-center gap-2">
          <span className="px-1 text-[11px] font-bold uppercase tracking-wide text-muted-foreground">
            Apartments
          </span>
          <Badge variant="secondary" className="gap-1.5">
            {activeApartments.length} active
          </Badge>
          <Badge variant="secondary" className="gap-1.5">
            {totalOccupants} occupants
          </Badge>
          <Badge
            variant={allocationComplete ? "success" : "warning"}
            className="gap-1.5"
          >
            {allocationComplete && <Check className="size-3" />}
            {totalM2Percentage.toFixed(2)}% m2 allocated
          </Badge>
          <Button onClick={handleNew} size="sm" className="ml-auto gap-2">
            <Plus className="size-4" />
            Add Apartment
          </Button>
        </div>
      </Card>

      <div className="grid gap-4 lg:grid-cols-[300px_minmax(0,1fr)]">
        <Card className="overflow-hidden">
          <div className="flex items-center justify-between border-b border-border px-4 py-3">
            <div className="text-sm font-semibold">Apartments</div>
          </div>
          <div className="divide-y divide-border">
            {apartments.map((apartment) => {
              const isSelected = !isNew && apartment.id === editing?.id;
              return (
                <button
                  type="button"
                  key={apartment.id}
                  className={cn(
                    "flex w-full items-center gap-3 px-4 py-3 text-left transition-colors hover:bg-accent/40",
                    isSelected && "bg-accent-soft text-accent-foreground",
                  )}
                  onClick={() => handleSelect(apartment)}
                >
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-sm font-semibold">
                      {apartment.label || "Unnamed"}
                    </div>
                    <div className="truncate font-mono text-xs text-muted-foreground">
                      {apartment.unit_code || "No unit"} -{" "}
                      {apartment.m2_percentage.toFixed(1)}%
                    </div>
                  </div>
                  <Badge
                    variant={apartment.is_active ? "success" : "secondary"}
                    className="h-5 px-2 text-[10px]"
                  >
                    {apartment.is_active ? "Active" : "Off"}
                  </Badge>
                </button>
              );
            })}
          </div>
          <div className="border-t border-border bg-surface-2 px-4 py-3">
            <div className="flex justify-between text-xs text-muted-foreground">
              <span>m2 allocated</span>
              <span
                className={cn(
                  "font-mono font-semibold",
                  allocationComplete ? "text-success" : "text-danger",
                )}
              >
                {totalM2Percentage.toFixed(1)}%
              </span>
            </div>
            <div className="mt-2 h-1.5 overflow-hidden rounded-full bg-surface-3">
              <div
                className={cn("h-full rounded-full", allocationComplete ? "bg-success" : "bg-danger")}
                style={{ width: `${Math.min(100, Math.max(0, totalM2Percentage))}%` }}
              />
            </div>
          </div>
        </Card>

        <ApartmentDetail
          apartment={editing}
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

function ApartmentDetail({
  apartment,
  isNew,
  isDirty,
  isSaving,
  isDeleting,
  onChange,
  onSave,
  onDiscard,
  onDelete,
}: {
  apartment: Apartment | null;
  isNew: boolean;
  isDirty: boolean;
  isSaving: boolean;
  isDeleting: boolean;
  onChange: (apartment: Apartment) => void;
  onSave: (event: FormEvent) => void;
  onDiscard: () => void;
  onDelete: () => void;
}) {
  if (!apartment) {
    return (
      <Card className="flex min-h-72 items-center justify-center p-8 text-center text-sm text-muted-foreground">
        Select an apartment to edit its billing and payer details.
      </Card>
    );
  }

  return (
    <Card className="overflow-hidden">
      <div className="border-b border-border px-5 py-3">
        <div>
          <h3 className="font-head text-lg font-semibold">
            {isNew ? "New apartment" : apartment.label || "Unnamed"}
            {!isNew && apartment.unit_code && (
              <span className="ml-2 font-body text-sm font-normal text-muted-foreground">
                {apartment.unit_code}
              </span>
            )}
          </h3>
        </div>
      </div>

      <form onSubmit={onSave}>
        <CardContent className="grid gap-6 p-5 lg:grid-cols-2">
          <div className="space-y-4 lg:border-r lg:border-border lg:pr-6">
            <div className="text-[11px] font-bold uppercase tracking-wide text-muted-foreground">
              Billing
            </div>
            <div className="grid grid-cols-2 gap-3">
              <Field label="Name">
                <Input
                  value={apartment.label}
                  onChange={(event) => onChange({ ...apartment, label: event.target.value })}
                  required
                />
              </Field>
              <Field label="Unit code">
                <Input
                  value={apartment.unit_code}
                  onChange={(event) =>
                    onChange({ ...apartment, unit_code: event.target.value })
                  }
                />
              </Field>
              <Field label="Occupants">
                <Input
                  type="number"
                  min={1}
                  value={apartment.occupant_count}
                  onChange={(event) =>
                    onChange({
                      ...apartment,
                      occupant_count: parseInt(event.target.value, 10) || 1,
                    })
                  }
                  required
                />
              </Field>
              <Field label="m2 %">
                <Input
                  type="number"
                  min={0}
                  step="0.01"
                  value={apartment.m2_percentage}
                  onChange={(event) =>
                    onChange({
                      ...apartment,
                      m2_percentage: parseFloat(event.target.value) || 0,
                    })
                  }
                  required
                />
              </Field>
            </div>
            <Field label="Contact email(s)">
              <Input
                value={apartment.contact_email}
                onChange={(event) =>
                  onChange({ ...apartment, contact_email: event.target.value })
                }
                placeholder="tenant@example.com, second@example.com"
              />
            </Field>
            <label className="flex items-center gap-2 text-sm">
              <input
                type="checkbox"
                checked={apartment.is_active}
                onChange={(event) =>
                  onChange({ ...apartment, is_active: event.target.checked })
                }
                className="size-4 accent-primary"
              />
              Active
            </label>
          </div>

          <div className="space-y-4">
            <div className="text-[11px] font-bold uppercase tracking-wide text-muted-foreground">
              Payer details (printed on UPN)
            </div>
            <Field label="Payer name">
              <Input
                value={apartment.payer_name}
                onChange={(event) =>
                  onChange({ ...apartment, payer_name: event.target.value })
                }
              />
            </Field>
            <Field label="Street address">
              <Input
                value={apartment.payer_address}
                onChange={(event) =>
                  onChange({ ...apartment, payer_address: event.target.value })
                }
              />
            </Field>
            <div className="grid grid-cols-[0.8fr_1.2fr] gap-3">
              <Field label="Postal code">
                <Input
                  value={apartment.payer_postal_code}
                  onChange={(event) =>
                    onChange({ ...apartment, payer_postal_code: event.target.value })
                  }
                />
              </Field>
              <Field label="City">
                <Input
                  value={apartment.payer_city}
                  onChange={(event) =>
                    onChange({ ...apartment, payer_city: event.target.value })
                  }
                />
              </Field>
            </div>
          </div>
        </CardContent>

        <div className="flex flex-wrap items-center gap-2 border-t border-border px-5 py-4">
          <Button type="submit" disabled={!isDirty || isSaving || isDeleting} className="gap-2">
            {isSaving ? (
              <Loader2 className="size-4 animate-spin" />
            ) : (
              <Save className="size-4" />
            )}
            {isNew ? "Save apartment" : "Save changes"}
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
