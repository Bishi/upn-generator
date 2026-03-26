import { createFileRoute } from "@tanstack/react-router";

export const Route = createFileRoute("/")({
  component: Dashboard,
});

function Dashboard() {
  return (
    <div>
      <h2 className="text-2xl font-bold mb-4">Dashboard</h2>
      <p className="text-muted-foreground">
        Welcome to UPN Generator. Start a new billing period or review existing ones.
      </p>
    </div>
  );
}
