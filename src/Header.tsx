import { useEffect, useRef } from "react";
import { Input } from "./components/ui/input";
interface HeaderProps {
  searchQuery: string;
  setSearchQuery: (query: string) => void;
}

export default function Header(props: HeaderProps) {
  const { searchQuery, setSearchQuery } = props;
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
          onChange={(e) => setSearchQuery(e.target.value)}
          className="text-xs placeholder:pl-2 border-0 focus-visible:outline-hidden focus-visible:ring-0 shadow-none"
        />
      </div>
    </div>
  );
}
