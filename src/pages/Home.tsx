import { useState, useEffect } from "react";
import TitleBar from "../Titlebar";
import { Input } from "../../components/ui/input";
import { cn } from "../../src/lib/utils";

// Define all searchable categories
export const searchCategories = [
  "Applications",
  "Calculator",
  "Contacts",
  "Conversion",
  "Definition",
  "Developer",
  "Documents",
  "Events & Reminders",
  "Folders",
  "Fonts",
  "Images",
  "Mail & Messages",
  "Movies",
  "Music",
  "Other",
  "PDF Documents",
  "Presentations",
  "Siri Suggestions",
  "Spreadsheets",
  "System Settings",
  "Tips",
  "Websites",
] as const;

export type SearchCategory = (typeof searchCategories)[number];

interface SearchResult {
  id: number;
  title: string;
  category: SearchCategory;
  icon?: React.ReactNode;
}

// Sample results - replace with your actual results logic
const results: SearchResult[] = [
  { id: 1, title: "Result 1", category: "Applications" },
  { id: 2, title: "Result 2", category: "Documents" },
  { id: 3, title: "Result 3", category: "Folders" },
];

export default function Home() {
  const [searchQuery, setSearchQuery] = useState<string>("");
  const [selectedCategories, setSelectedCategories] = useState<
    Set<SearchCategory>
  >(new Set(searchCategories));
  const [selectedResultIndex, setSelectedResultIndex] = useState<number>(0); // Changed name here

  const toggleCategory = (category: SearchCategory) => {
    setSelectedCategories((prev) => {
      const newSet = new Set(prev);
      if (newSet.has(category)) {
        newSet.delete(category);
      } else {
        newSet.add(category);
      }
      return newSet;
    });
  };

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setSelectedResultIndex((current) =>
          current >= results.length - 1 ? 0 : current + 1
        );
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        setSelectedResultIndex((current) =>
          current <= 0 ? results.length - 1 : current - 1
        );
      } else if (e.key === "Enter") {
        e.preventDefault();
        const selectedItem = results[selectedResultIndex]; // Changed name here
        if (selectedItem) {
          // Handle result selection
        }
      }
    };

    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [results, selectedResultIndex]);

  return (
    <div className="flex flex-col h-full">
      <TitleBar
        selectedCategories={selectedCategories}
        toggleCategory={toggleCategory}
        searchCategories={searchCategories}
      />

      <div className="flex flex-col max-w-2xl mx-auto mt-8 w-full bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/60 rounded-xl border shadow-xl">
        <div className="p-3 border-b">
          <Input
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="border-none bg-transparent focus-visible:ring-0 focus-visible:ring-offset-0 placeholder:text-muted-foreground/60"
            placeholder="Search your computer"
          />
        </div>
        <SearchResults
          results={results}
          selectedId={selectedResultIndex}
          onSelect={setSelectedResultIndex}
        />
      </div>
    </div>
  );
}

interface SearchResultsProps {
  results: SearchResult[];
  selectedId: number;
  onSelect: (id: number) => void;
}

function SearchResults(props: SearchResultsProps) {
  const { results, selectedId, onSelect } = props;

  return (
    <div className="p-2 max-h-[60vh] overflow-auto">
      {results.map((result) => (
        <div
          key={result.id}
          onClick={() => onSelect(result.id)}
          className={cn(
            "flex items-center gap-3 px-3 py-2 rounded-md cursor-pointer",
            selectedId === result.id
              ? "bg-primary text-primary-foreground"
              : "hover:bg-muted"
          )}
        >
          {result.icon}
          <div className="flex flex-col">
            <span className="text-sm font-medium">{result.title}</span>
            <span className="text-xs text-muted-foreground">
              {result.category}
            </span>
          </div>
        </div>
      ))}
    </div>
  );
}
