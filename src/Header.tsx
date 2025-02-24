import { useEffect, useRef } from "react";
import { Input } from "./components/ui/input";
import { CommandShortcut } from "./components/ui/command";

interface HeaderProps {
  searchQuery: string;
  handleSearch: (query: string) => Promise<void>;
}

export default function Header(props: HeaderProps) {
  const { searchQuery, handleSearch } = props;
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  return (
    <div className="sticky top-0 flex flex-col gap-2 border-b border-b-border">
      <div className="py-2 flex flex-row items-center justify-between">
        <Input
          placeholder="Type a command or search..."
          value={searchQuery}
          ref={inputRef}
          autoFocus
          onChange={(e) => handleSearch(e.target.value)}
          className="text-xs placeholder:pl-2 border-0 focus-visible:outline-hidden focus-visible:ring-0 shadow-none"
        />
        <CommandShortcut className="mr-4 border border-border px-1 py-[2px] rounded-lg text-[10px] bg-gray-200 dark:bg-zinc-950 text-primary-foreground/80">
          ⌘+space
        </CommandShortcut>
      </div>
    </div>
  );
}
