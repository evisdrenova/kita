import { Search } from "lucide-react";

export default function EmptyState() {
  return (
    <div className="flex flex-col items-center justify-center p-8 space-y-2 text-primary-foreground/40 rounded-xl">
      <div className="bg-gray-200 dark:bg-muted rounded-full p-4">
        <Search className="text-primary-foreground/80" />
      </div>
      <h2 className="mb-2 text-xs font-semibold text-primary-foreground">
        No files found
      </h2>
      <p>Try searching for something else</p>
    </div>
  );
}
