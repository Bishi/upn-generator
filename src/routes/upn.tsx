import { createFileRoute } from "@tanstack/react-router";

export const Route = createFileRoute("/upn")({
  component: UpnPreview,
});

function UpnPreview() {
  return (
    <div>
      <h2 className="text-2xl font-bold mb-4">UPN Preview</h2>
      <p className="text-muted-foreground">Preview and send generated UPN payment forms.</p>
    </div>
  );
}
