import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { useState, useEffect } from "react";
import { Check, Loader2, Save } from "lucide-react";
import { ipc } from "@/lib/ipc";
import { useWorkflowSnapshotContext } from "@/lib/workflow-snapshot";
import type { Building } from "@/lib/types";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Card, CardContent } from "@/components/ui/card";
import { SettingsLoadingCard } from "@/components/settings/SettingsLoadingCard";
import { SETTINGS_PANEL_WIDTH } from "@/components/settings/layout";

const emptyBuilding: Building = {
  id: null,
  name: "",
  address: "",
  city: "",
  postal_code: "",
};

export function BuildingSection() {
  const queryClient = useQueryClient();
  const snapshot = useWorkflowSnapshotContext();
  const { data: building, isLoading } = useQuery({
    queryKey: ["building"],
    queryFn: ipc.getBuilding,
    initialData: snapshot.building ?? undefined,
  });

  const [form, setForm] = useState<Building>(emptyBuilding);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    if (building) setForm(building);
  }, [building]);

  const mutation = useMutation({
    mutationFn: ipc.saveBuilding,
    onSuccess: async (updated) => {
      queryClient.setQueryData(["building"], updated);
      await snapshot.refresh({ core: true, periods: false, selected: true, statuses: true });
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    },
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    mutation.mutate(form);
  };

  if (isLoading) return <SettingsLoadingCard />;

  return (
    <Card className={`${SETTINGS_PANEL_WIDTH} overflow-hidden`}>
      <div className="border-b border-border px-5 py-4">
        <h3 className="font-head text-lg font-semibold">Building Details</h3>
        <p className="mt-1 text-sm text-muted-foreground">
          Used as the payer address on UPN forms.
        </p>
      </div>
      <CardContent className="p-5">
        <form onSubmit={handleSubmit} className="space-y-4">
          <div className="space-y-1.5">
            <Label htmlFor="name">Building name</Label>
            <Input
              id="name"
              value={form.name}
              onChange={(e) => setForm({ ...form, name: e.target.value })}
              placeholder="e.g. Skupnost stanovalcev Kamniška 36"
            />
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="address">Street address</Label>
            <Input
              id="address"
              value={form.address}
              onChange={(e) => setForm({ ...form, address: e.target.value })}
              placeholder="e.g. Kamniška ulica 36"
            />
          </div>
          <div className="grid grid-cols-2 gap-3">
            <div className="space-y-1.5">
              <Label htmlFor="postal_code">Postal code</Label>
              <Input
                id="postal_code"
                value={form.postal_code}
                onChange={(e) => setForm({ ...form, postal_code: e.target.value })}
                placeholder="1000"
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="city">City</Label>
              <Input
                id="city"
                value={form.city}
                onChange={(e) => setForm({ ...form, city: e.target.value })}
                placeholder="Ljubljana"
              />
            </div>
          </div>
          <Button type="submit" disabled={mutation.isPending} className="gap-2">
            {mutation.isPending ? (
              <Loader2 className="size-4 animate-spin" />
            ) : saved ? (
              <Check className="size-4" />
            ) : (
              <Save className="size-4" />
            )}
            Save
          </Button>
        </form>
      </CardContent>
    </Card>
  );
}
