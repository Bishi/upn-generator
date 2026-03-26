import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { Plus, Pencil, Trash2, Users, X, Save } from "lucide-react";
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
  occupant_count: 1,
  contact_email: "",
  payer_name: "",
  payer_address: "",
  payer_city: "Ljubljana",
  payer_postal_code: "1000",
  is_active: true,
});

export function ApartmentsSection() {
  const queryClient = useQueryClient();
  const { data: apartments = [], isLoading } = useQuery({
    queryKey: ["apartments"],
    queryFn: ipc.getApartments,
  });

  const [editing, setEditing] = useState<Apartment | null>(null);
  const [isNew, setIsNew] = useState(false);

  const totalOccupants = apartments.filter((a) => a.is_active).reduce((s, a) => s + a.occupant_count, 0);

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
    setEditing(newApartment());
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
            {apartments.length} apartment{apartments.length !== 1 ? "s" : ""} · {totalOccupants} total occupants
          </p>
        </div>
        <Button onClick={handleNew} size="sm" className="gap-2">
          <Plus className="size-4" />
          Add Apartment
        </Button>
      </div>

      {/* Cards grid */}
      <div className="grid grid-cols-2 gap-4">
        {apartments.map((apt) => (
          <Card key={apt.id} className={cn(!apt.is_active && "opacity-60")}>
            <CardHeader className="pb-2">
              <CardTitle className="text-base flex items-center justify-between">
                <span>{apt.label || "Unnamed"}</span>
                <Badge variant={apt.is_active ? "default" : "secondary"}>
                  {apt.is_active ? "Active" : "Inactive"}
                </Badge>
              </CardTitle>
            </CardHeader>
            <CardContent className="text-sm space-y-1 text-muted-foreground">
              <div className="flex items-center gap-1.5">
                <Users className="size-3.5" />
                <span>{apt.occupant_count} occupant{apt.occupant_count !== 1 ? "s" : ""}</span>
                {totalOccupants > 0 && apt.is_active && (
                  <span className="ml-auto text-xs">
                    {((apt.occupant_count / totalOccupants) * 100).toFixed(1)}%
                  </span>
                )}
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

      {/* Edit / New form modal */}
      {editing && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <Card className="w-full max-w-md max-h-[90vh] overflow-y-auto">
            <CardHeader>
              <CardTitle>{isNew ? "Add Apartment" : "Edit Apartment"}</CardTitle>
            </CardHeader>
            <form onSubmit={handleSave}>
              <CardContent className="space-y-4">
                <div className="grid grid-cols-2 gap-3">
                  <div className="space-y-1.5">
                    <Label>Label</Label>
                    <Input
                      value={editing.label}
                      onChange={(e) => setEditing({ ...editing, label: e.target.value })}
                      placeholder="Apt 1"
                      required
                    />
                  </div>
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
                </div>
                <div className="space-y-1.5">
                  <Label>Contact email (receives UPNs)</Label>
                  <Input
                    type="email"
                    value={editing.contact_email}
                    onChange={(e) => setEditing({ ...editing, contact_email: e.target.value })}
                    placeholder="tenant@example.com"
                  />
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
                    placeholder="Kamniška ulica 36/1"
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
