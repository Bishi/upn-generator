import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { Plus, Pencil, Trash2, Users, X, Save, Percent } from "lucide-react";
import { ipc } from "@/lib/ipc";
import type { Apartment } from "@/lib/types";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Card, CardContent, CardHeader, CardTitle, CardFooter } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
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

export function ApartmentsSection() {
  const queryClient = useQueryClient();
  const { data: apartments = [], isLoading } = useQuery({
    queryKey: ["apartments"],
    queryFn: ipc.getApartments,
  });
  const { data: building } = useQuery({
    queryKey: ["building"],
    queryFn: ipc.getBuilding,
  });

  const [editing, setEditing] = useState<Apartment | null>(null);
  const [isNew, setIsNew] = useState(false);

  const totalOccupants = apartments.filter((a) => a.is_active).reduce((s, a) => s + a.occupant_count, 0);
  const totalM2Percentage = apartments.filter((a) => a.is_active).reduce((s, a) => s + a.m2_percentage, 0);

  const saveMutation = useMutation({
    mutationFn: ipc.saveApartment,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["apartments"] });
      setEditing(null);
      setIsNew(false);
    },
  });

  const deleteMutation = useMutation({
    mutationFn: ipc.deleteApartment,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["apartments"] }),
  });

  const handleEdit = (apt: Apartment) => {
    setEditing({ ...apt });
    setIsNew(false);
  };

  const handleNew = () => {
    setEditing({
      ...newApartment(),
      payer_address: building?.address ?? "",
      payer_city: building?.city ?? "Ljubljana",
      payer_postal_code: building?.postal_code ?? "1000",
    });
    setIsNew(true);
  };

  const handleSave = (e: React.FormEvent) => {
    e.preventDefault();
    if (editing) saveMutation.mutate(editing);
  };

  if (isLoading) return <div className="text-muted-foreground text-sm">Loading...</div>;

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <p className="text-sm text-muted-foreground">
            {apartments.length} apartment{apartments.length !== 1 ? "s" : ""} · {totalOccupants} total occupants · {totalM2Percentage.toFixed(2)}% active m² share
          </p>
        </div>
        <Button onClick={handleNew} size="sm" className="gap-2">
          <Plus className="size-4" />
          Add Apartment
        </Button>
      </div>

      <div className="grid grid-cols-2 gap-4">
        {apartments.map((apt) => (
          <Card key={apt.id} className={cn(!apt.is_active && "opacity-60")}>
            <CardHeader className="pb-2">
              <CardTitle className="text-base flex items-center justify-between gap-3">
                <div className="min-w-0">
                  <div className="truncate">{apt.label || "Unnamed"}</div>
                  <div className="text-xs font-normal text-muted-foreground">
                    {apt.unit_code || "No unit code"}
                  </div>
                </div>
                <Badge variant={apt.is_active ? "default" : "secondary"}>
                  {apt.is_active ? "Active" : "Inactive"}
                </Badge>
              </CardTitle>
            </CardHeader>
            <CardContent className="text-sm space-y-2 text-muted-foreground">
              <div className="flex items-center gap-1.5">
                <Users className="size-3.5" />
                <span>{apt.occupant_count} occupant{apt.occupant_count !== 1 ? "s" : ""}</span>
                {totalOccupants > 0 && apt.is_active && (
                  <span className="ml-auto text-xs">
                    {((apt.occupant_count / totalOccupants) * 100).toFixed(1)}% people
                  </span>
                )}
              </div>
              <div className="flex items-center gap-1.5">
                <Percent className="size-3.5" />
                <span>{apt.m2_percentage.toFixed(2)}% m² share</span>
              </div>
              <div className="truncate">{apt.contact_email || "—"}</div>
              <div className="font-medium text-foreground">{apt.payer_name || "—"}</div>
              <div className="text-xs">{apt.payer_address}, {apt.payer_postal_code} {apt.payer_city}</div>
            </CardContent>
            <CardFooter className="gap-2 pt-0">
              <Button variant="outline" size="sm" onClick={() => handleEdit(apt)} className="gap-1.5">
                <Pencil className="size-3.5" />
                Edit
              </Button>
              <Button
                variant="outline"
                size="sm"
                onClick={() => apt.id && deleteMutation.mutate(apt.id)}
                className="gap-1.5 text-destructive hover:text-destructive"
              >
                <Trash2 className="size-3.5" />
                Delete
              </Button>
            </CardFooter>
          </Card>
        ))}
      </div>

      {editing && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <Card className="w-full max-w-md max-h-[90vh] flex flex-col">
            <CardHeader className="shrink-0">
              <CardTitle>{isNew ? "Add Apartment" : "Edit Apartment"}</CardTitle>
            </CardHeader>
            <form onSubmit={handleSave} className="flex flex-col overflow-hidden flex-1">
              <CardContent className="space-y-4 overflow-y-auto flex-1">
                <div className="grid grid-cols-2 gap-3">
                  <div className="space-y-1.5">
                    <Label>Apartment name</Label>
                    <Input
                      value={editing.label}
                      onChange={(e) => setEditing({ ...editing, label: e.target.value })}
                      placeholder="Andreja Vidonja"
                      required
                    />
                  </div>
                  <div className="space-y-1.5">
                    <Label>Unit code</Label>
                    <Input
                      value={editing.unit_code}
                      onChange={(e) => setEditing({ ...editing, unit_code: e.target.value })}
                      placeholder="1287/6"
                    />
                  </div>
                </div>
                <div className="grid grid-cols-2 gap-3">
                  <div className="space-y-1.5">
                    <Label>Occupants</Label>
                    <Input
                      type="number"
                      min={1}
                      value={editing.occupant_count}
                      onChange={(e) => setEditing({ ...editing, occupant_count: parseInt(e.target.value) || 1 })}
                      required
                    />
                  </div>
                  <div className="space-y-1.5">
                    <Label>m² percentage</Label>
                    <Input
                      type="number"
                      min={0}
                      step="0.01"
                      value={editing.m2_percentage}
                      onChange={(e) => setEditing({ ...editing, m2_percentage: parseFloat(e.target.value) || 0 })}
                      required
                    />
                  </div>
                </div>
                <div className="space-y-1.5">
                  <Label>Contact email(s)</Label>
                  <Input
                    value={editing.contact_email}
                    onChange={(e) => setEditing({ ...editing, contact_email: e.target.value })}
                    placeholder="tenant@example.com, second@example.com"
                  />
                  <p className="text-xs text-muted-foreground">
                    Separate multiple recipients with commas. One combined apartment PDF will be sent to all of them.
                  </p>
                </div>
                <div className="space-y-1.5 border-t pt-4">
                  <p className="text-xs font-medium text-muted-foreground uppercase tracking-wide">Payer details (printed on UPN)</p>
                </div>
                <div className="space-y-1.5">
                  <Label>Payer name</Label>
                  <Input
                    value={editing.payer_name}
                    onChange={(e) => setEditing({ ...editing, payer_name: e.target.value })}
                    placeholder="Ana Horvat"
                  />
                </div>
                <div className="space-y-1.5">
                  <Label>Street address</Label>
                  <Input
                    value={editing.payer_address}
                    onChange={(e) => setEditing({ ...editing, payer_address: e.target.value })}
                    placeholder="Kamniska ulica 36/1"
                  />
                </div>
                <div className="grid grid-cols-2 gap-3">
                  <div className="space-y-1.5">
                    <Label>Postal code</Label>
                    <Input
                      value={editing.payer_postal_code}
                      onChange={(e) => setEditing({ ...editing, payer_postal_code: e.target.value })}
                      placeholder="1000"
                    />
                  </div>
                  <div className="space-y-1.5">
                    <Label>City</Label>
                    <Input
                      value={editing.payer_city}
                      onChange={(e) => setEditing({ ...editing, payer_city: e.target.value })}
                      placeholder="Ljubljana"
                    />
                  </div>
                </div>
                <div className="flex items-center gap-2">
                  <input
                    type="checkbox"
                    id="is_active"
                    checked={editing.is_active}
                    onChange={(e) => setEditing({ ...editing, is_active: e.target.checked })}
                    className="size-4"
                  />
                  <Label htmlFor="is_active">Active (included in splits)</Label>
                </div>
              </CardContent>
              <CardFooter className="gap-2">
                <Button type="submit" disabled={saveMutation.isPending} className="gap-2">
                  <Save className="size-4" />
                  {saveMutation.isPending ? "Saving..." : "Save"}
                </Button>
                <Button type="button" variant="outline" onClick={() => { setEditing(null); setIsNew(false); }} className="gap-2">
                  <X className="size-4" />
                  Cancel
                </Button>
              </CardFooter>
            </form>
          </Card>
        </div>
      )}
    </div>
  );
}
