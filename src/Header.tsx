import { useEffect, useRef } from "react";
import { Input } from "./components/ui/input";

interface HeaderProps {
  searchQuery: string;
  setSearchQuery: (query: string) => void;
}

export default function Header(props: HeaderProps) {
  const { searchQuery, setSearchQuery } = props;
  const inputRef = useRef<HTMLInputElement>(null);
  const wrapperRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  // Apply the draggable region using setAttribute which bypasses TypeScript's type checking
  useEffect(() => {
    if (wrapperRef.current) {
      wrapperRef.current.setAttribute("data-tauri-drag-region", "");
    }
  }, []);

  return (
    <div
      ref={wrapperRef}
      className="sticky top-0 flex flex-col gap-2 border-b border-b-border p-2"
      data-tauri-drag-region=""
    >
      <div
        className="flex flex-row items-center justify-between"
        data-tauri-drag-region=""
      >
        <Input
          placeholder="Type a command or search..."
          value={searchQuery}
          autoCorrect="off"
          spellCheck="false"
          ref={inputRef}
          autoFocus
          onChange={(e) => setSearchQuery(e.target.value)}
          className="text-lg placeholder:pl-2 border-0 focus-visible:outline-hidden focus-visible:ring-0 shadow-none"
        />
      </div>
    </div>
  );
}
