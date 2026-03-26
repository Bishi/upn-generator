import { createFileRoute } from "@tanstack/react-router";

export const Route = createFileRoute("/splits")({
  component: Splits,
});

function Splits() {
  return (
    <div>
      <h2 className="text-2xl font-bold mb-4">Splits</h2>
      <p className="text-muted-foreground">Review bill splits across apartments.</p>
    </div>
  );
}
