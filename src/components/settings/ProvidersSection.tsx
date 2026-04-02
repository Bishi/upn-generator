import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { Plus, Pencil, Trash2, X, Save, ChevronDown, ChevronUp } from "lucide-react";
import { ipc } from "@/lib/ipc";
import type { Provider } from "@/lib/types";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Card, CardContent, CardHeader, CardTitle, CardFooter } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";

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
      return "People";
    case "equal_apartments":
      return "Equal";
    default:
      return "m\u00B2";
  }
}

export function ProvidersSection() {
  const queryClient = useQueryClient();
  const { data: providers = [], isLoading } = useQuery({
    queryKey: ["providers"],
    queryFn: ipc.getProviders,
  });

  const [editing, setEditing] = useState<Provider | null>(null);
  const [isNew, setIsNew] = useState(false);
  const [showPatterns, setShowPatterns] = useState(false);

  const saveMutation = useMutation({
    mutationFn: ipc.saveProvider,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["providers"] });
      setEditing(null);
      setIsNew(false);
    },
  });

  const deleteMutation = useMutation({
    mutationFn: ipc.deleteProvider,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["providers"] }),
  });

  const handleSave = (e: React.FormEvent) => {
    e.preventDefault();
    if (editing) saveMutation.mutate(editing);
  };

  if (isLoading) return <div className="text-muted-foreground text-sm">Loading...</div>;

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <p className="text-sm text-muted-foreground">
          {providers.length} provider{providers.length !== 1 ? "s" : ""} configured
        </p>
        <Button onClick={() => { setEditing(newProvider()); setIsNew(true); }} size="sm" className="gap-2">
          <Plus className="size-4" />
          Add Provider
        </Button>
      </div>

      <div className="space-y-2">
        {providers.map((p) => (
          <Card key={p.id}>
            <CardContent className="py-3 px-4 flex items-center gap-4">
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2">
                  <span className="font-medium text-sm">{p.name}</span>
                  <Badge variant="outline" className="text-xs">{p.purpose_code}</Badge>
                  <Badge variant="secondary" className="text-xs">
                    {splitBasisLabel(p.split_basis)}
                  </Badge>
                  {p.service_type && (
                    <span className="text-xs text-muted-foreground">{p.service_type}</span>
                  )}
                </div>
                <div className="text-xs text-muted-foreground truncate mt-0.5">
                  {p.creditor_iban || "No IBAN set"}
                </div>
              </div>
              <div className="flex gap-2 shrink-0">
                <Button variant="outline" size="sm" onClick={() => { setEditing({ ...p }); setIsNew(false); }} className="gap-1.5">
                  <Pencil className="size-3.5" />
                  Edit
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => p.id && deleteMutation.mutate(p.id)}
                  className="gap-1.5 text-destructive hover:text-destructive"
                >
                  <Trash2 className="size-3.5" />
                </Button>
              </div>
            </CardContent>
          </Card>
        ))}

        {providers.length === 0 && (
          <div className="text-center py-8 text-muted-foreground text-sm border border-dashed rounded-lg">
            No providers yet. Add your first provider to get started.
          </div>
        )}
      </div>

      {editing && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <Card className="w-full max-w-xl max-h-[90vh] flex flex-col">
            <CardHeader className="shrink-0">
              <CardTitle>{isNew ? "Add Provider" : "Edit Provider"}</CardTitle>
            </CardHeader>
            <form onSubmit={handleSave} className="flex flex-col overflow-hidden flex-1">
              <CardContent className="space-y-4 overflow-y-auto flex-1">
                <div className="grid grid-cols-2 gap-3">
                  <div className="space-y-1.5">
                    <Label>Provider name</Label>
                    <Input
                      value={editing.name}
                      onChange={(e) => setEditing({ ...editing, name: e.target.value })}
                      placeholder="Elektro energija"
                      required
                    />
                  </div>
                  <div className="space-y-1.5">
                    <Label>Service type</Label>
                    <Input
                      value={editing.service_type}
                      onChange={(e) => setEditing({ ...editing, service_type: e.target.value })}
                      placeholder="Electricity"
                    />
                  </div>
                </div>
                <div className="space-y-1.5">
                  <Label>Split basis</Label>
                  <select
                    value={editing.split_basis}
                    onChange={(e) => setEditing({ ...editing, split_basis: e.target.value as Provider["split_basis"] })}
                    className="flex h-9 w-full rounded-md border border-input bg-transparent px-3 py-1 text-sm shadow-sm focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
                  >
                    <option value="m2_percentage">{"m\u00B2"}</option>
                    <option value="occupants">People</option>
                    <option value="equal_apartments">Equal</option>
                  </select>
                </div>

                <div className="space-y-1.5 border-t pt-4">
                  <p className="text-xs font-medium text-muted-foreground uppercase tracking-wide">Creditor details</p>
                </div>
                <div className="space-y-1.5">
                  <Label>Creditor name</Label>
                  <Input
                    value={editing.creditor_name}
                    onChange={(e) => setEditing({ ...editing, creditor_name: e.target.value })}
                    placeholder="Elektro energija d.o.o."
                  />
                </div>
                <div className="space-y-1.5">
                  <Label>Creditor address</Label>
                  <Input
                    value={editing.creditor_address}
                    onChange={(e) => setEditing({ ...editing, creditor_address: e.target.value })}
                    placeholder="Dunajska cesta 119"
                  />
                </div>
                <div className="grid grid-cols-2 gap-3">
                  <div className="space-y-1.5">
                    <Label>Postal code</Label>
                    <Input
                      value={editing.creditor_postal_code}
                      onChange={(e) => setEditing({ ...editing, creditor_postal_code: e.target.value })}
                      placeholder="1000"
                    />
                  </div>
                  <div className="space-y-1.5">
                    <Label>City</Label>
                    <Input
                      value={editing.creditor_city}
                      onChange={(e) => setEditing({ ...editing, creditor_city: e.target.value })}
                      placeholder="Ljubljana"
                    />
                  </div>
                </div>
                <div className="grid grid-cols-2 gap-3">
                  <div className="space-y-1.5">
                    <Label>IBAN</Label>
                    <Input
                      value={editing.creditor_iban}
                      onChange={(e) => setEditing({ ...editing, creditor_iban: e.target.value })}
                      placeholder="SI56 0400 1004 8988 093"
                    />
                  </div>
                  <div className="space-y-1.5">
                    <Label>Purpose code</Label>
                    <select
                      value={editing.purpose_code}
                      onChange={(e) => setEditing({ ...editing, purpose_code: e.target.value })}
                      className="flex h-9 w-full rounded-md border border-input bg-transparent px-3 py-1 text-sm shadow-sm focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
                    >
                      {PURPOSE_CODES.map((c) => (
                        <option key={c} value={c}>{c}</option>
                      ))}
                    </select>
                  </div>
                </div>

                <button
                  type="button"
                  onClick={() => setShowPatterns(!showPatterns)}
                  className="flex items-center gap-2 text-sm font-medium border-t pt-4 w-full text-left"
                >
                  {showPatterns ? <ChevronUp className="size-4" /> : <ChevronDown className="size-4" />}
                  Parsing patterns (regex)
                </button>

                {showPatterns && (
                  <div className="space-y-3">
                    <p className="text-xs text-muted-foreground">
                      Regex patterns to extract data from the bill&apos;s text. Use capture group 1 for the value.
                    </p>
                    {[
                      { key: "match_pattern", label: "Match pattern (identifies this provider)", placeholder: "Elektro energija" },
                      { key: "amount_pattern", label: "Amount pattern", placeholder: "ZA PLACILO.*?([\\d,\\.]+)\\s*EUR" },
                      { key: "reference_pattern", label: "Reference pattern", placeholder: "SI\\d{2}\\s+[\\d\\s]+" },
                      { key: "due_date_pattern", label: "Due date pattern", placeholder: "Rok placila:\\s*([\\d\\.]+)" },
                      { key: "invoice_number_pattern", label: "Invoice number pattern", placeholder: "Racun.*?([A-Z0-9\\-]+)" },
                      { key: "purpose_text_template", label: "Purpose text template", placeholder: "rn. {invoice} ({month}-{year})" },
                    ].map(({ key, label, placeholder }) => (
                      <div key={key} className="space-y-1.5">
                        <Label className="text-xs">{label}</Label>
                        <Input
                          value={(editing as unknown as Record<string, string>)[key]}
                          onChange={(e) => setEditing({ ...editing, [key]: e.target.value })}
                          placeholder={placeholder}
                          className="font-mono text-xs"
                        />
                      </div>
                    ))}
                  </div>
                )}
              </CardContent>
              <CardFooter className="gap-2">
                <Button type="submit" disabled={saveMutation.isPending} className="gap-2">
                  <Save className="size-4" />
                  {saveMutation.isPending ? "Saving..." : "Save"}
                </Button>
                <Button type="button" variant="outline" onClick={() => { setEditing(null); setIsNew(false); setShowPatterns(false); }} className="gap-2">
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
