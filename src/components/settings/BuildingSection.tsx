import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { useState, useEffect } from "react";
import { Save } from "lucide-react";
import { ipc } from "@/lib/ipc";
import type { Building } from "@/lib/types";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";

const emptyBuilding: Building = {
  id: null,
  name: "",
  address: "",
  city: "",
  postal_code: "",
};

export function BuildingSection() {
  const queryClient = useQueryClient();
  const { data: building, isLoading } = useQuery({
    queryKey: ["building"],
    queryFn: ipc.getBuilding,
  });

  const [form, setForm] = useState<Building>(emptyBuilding);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    if (building) setForm(building);
  }, [building]);

  const mutation = useMutation({
    mutationFn: ipc.saveBuilding,
    onSuccess: (updated) => {
      queryClient.setQueryData(["building"], updated);
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    },
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    mutation.mutate(form);
  };

  if (isLoading) return <div className="text-muted-foreground text-sm">Loading...</div>;

  return (
    <Card className="max-w-lg">
      <CardHeader>
        <CardTitle>Building Details</CardTitle>
        <CardDescription>The building this app manages. Used as creditor address on UPN forms.</CardDescription>
      </CardHeader>
      <CardContent>
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
            <Save className="size-4" />
            {saved ? "Saved!" : mutation.isPending ? "Saving..." : "Save"}
          </Button>
        </form>
      </CardContent>
    </Card>
  );
}
