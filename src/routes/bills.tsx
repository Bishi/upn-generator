import { createFileRoute } from "@tanstack/react-router";

export const Route = createFileRoute("/bills")({
  component: Bills,
});

function Bills() {
  return (
    <div>
      <h2 className="text-2xl font-bold mb-4">Bills</h2>
      <p className="text-muted-foreground">Import and review utility bills.</p>
    </div>
  );
}
