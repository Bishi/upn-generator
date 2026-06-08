import { Card, CardContent, CardHeader } from "@/components/ui/card";
import { cn } from "@/lib/utils";

export function SettingsLoadingCard({
  className,
  rows = 4,
}: {
  className?: string;
  rows?: number;
}) {
  return (
    <Card className={cn("max-w-lg", className)}>
      <CardHeader>
        <div className="h-5 w-44 rounded-md bg-surface-3" />
        <div className="mt-2 h-4 w-72 max-w-full rounded-md bg-surface-3" />
      </CardHeader>
      <CardContent className="space-y-4">
        {Array.from({ length: rows }).map((_, index) => (
          <div key={index} className="space-y-2">
            <div className="h-3 w-24 rounded-md bg-surface-3" />
            <div className="h-9 rounded-md bg-surface-3" />
          </div>
        ))}
      </CardContent>
    </Card>
  );
}
