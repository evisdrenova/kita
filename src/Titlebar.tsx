import { RiExpandUpDownFill } from "react-icons/ri";
import { IoSearchOutline } from "react-icons/io5";
import { useState, useEffect } from "react";

import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
  SheetTrigger,
} from "../components/ui/sheet";

import { VisuallyHidden } from "@radix-ui/react-visually-hidden";
import { ThemeToggle } from "../src/ThemeProvider";
import { Button } from "../components/ui/button";
import { Settings, Settings2 } from "lucide-react";
import { Checkbox } from "../components/ui/checkbox";

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

interface Props {
  selectedCategories: Set<SearchCategory>;
  toggleCategory: (category: SearchCategory) => void;
}

export default function TitleBar(props: Props) {
  const { selectedCategories, toggleCategory } = props;
  const handleClose = () => window.electron.closeWindow();
  const handleMinimize = () => window.electron.minimizeWindow();
  const handleMaximize = () => window.electron.maximizeWindow();

  const [open, setOpen] = useState(false);

  useEffect(() => {
    const down = (e: KeyboardEvent) => {
      if (e.key === "s" && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        setOpen((open) => !open);
      }
    };

    document.addEventListener("keydown", down);
    return () => document.removeEventListener("keydown", down);
  }, []);

  const handleSearch = (e: React.MouseEvent) => {
    e.preventDefault();
    setOpen((open) => !open);
  };

  return (
    <div className="h-8 bg-background flex justify-between items-center select-none dragable px-3">
      <WindowAction
        handleClose={handleClose}
        handleMinimize={handleMinimize}
        handleMaximize={handleMaximize}
      />
      <div className="no-drag">Kita</div>
      <div className="flex flex-row items-center rounded-lg no-drag ">
        <ThemeToggle />
        <FolderSettings
          selectedCategories={selectedCategories}
          toggleCategory={toggleCategory}
        />
      </div>
    </div>
  );
}

interface WindowActionsProps {
  handleClose: () => void;
  handleMinimize: () => void;
  handleMaximize: () => void;
}

function WindowAction(props: WindowActionsProps) {
  const { handleClose, handleMinimize, handleMaximize } = props;
  return (
    <div
      className="flex items-center gap-2  no-drag group" // Added group class
    >
      <button
        onClick={handleClose}
        className="w-3 h-3 rounded-full bg-red-500 hover:bg-red-600 text-gray-900 flex items-center justify-center text-xs no-drag"
        title="Close"
      >
        <span className="opacity-0 text-[9px] group-hover:opacity-100">✕</span>
      </button>
      <button
        onClick={handleMinimize}
        className="w-3 h-3 rounded-full bg-yellow-500 hover:bg-yellow-600 flex items-center justify-center text-gray-900 text-xs no-drag"
        title="Minimize"
      >
        <span className="opacity-0 group-hover:opacity-100">−</span>
      </button>
      <button
        onClick={handleMaximize}
        className="w-3 h-3 rounded-full bg-green-500 hover:bg-green-600 flex items-center justify-center text-gray-900 text-xs no-drag"
        title="Maximize"
      >
        <span className="opacity-0 group-hover:opacity-100 -rotate-45">
          <RiExpandUpDownFill />
        </span>
      </button>
    </div>
  );
}

interface FolderSettingsProps {
  toggleCategory: (category: SearchCategory) => void;
  selectedCategories: Set<SearchCategory>;
}

function FolderSettings(props: FolderSettingsProps) {
  const { toggleCategory, selectedCategories } = props;
  return (
    <Sheet>
      <SheetTrigger asChild>
        <Button variant="ghost" size="icon" className="ml-2">
          <Settings className="h-4 w-4" />
        </Button>
      </SheetTrigger>
      <SheetContent side="right" className="w-[400px] sm:w-[540px]">
        <SheetHeader>
          <SheetTitle> Folders</SheetTitle>
          <SheetDescription>
            Only selected folders will appear in search results.
          </SheetDescription>
        </SheetHeader>
        <div className="mt-6 space-y-4">
          {searchCategories.map((category) => (
            <div key={category} className="flex items-center space-x-2">
              <Checkbox
                id={category}
                checked={selectedCategories.has(category)}
                onCheckedChange={() => toggleCategory(category)}
              />
              <label
                htmlFor={category}
                className="text-sm font-medium leading-none peer-disabled:cursor-not-allowed peer-disabled:opacity-70"
              >
                {category}
              </label>
            </div>
          ))}
        </div>
      </SheetContent>
    </Sheet>
  );
}
