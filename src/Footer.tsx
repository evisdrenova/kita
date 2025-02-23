import { ArrowDown, ArrowUp, CornerDownLeft, Folder } from "lucide-react";
import { Button } from "@/src/components/ui/button";
import { ThemeToggle } from "./ThemeProvider";

interface FooterProps {
  setIsSettingsOpen: (val: boolean) => void;
}

export default function Footer(props: FooterProps) {
  const { setIsSettingsOpen } = props;
  return (
    <div className="h-8 flex justify-between items-center px-3 my-1 border-t border-t-border">
      <div className="flex flex-row items-center gap-4 text-primary-foreground/80">
        <div className="flex flex-row items-center gap-1 text-xs">
          <div className=" border border-border p-1 rounded-lg bg-gray-200 dark:bg-zinc-950">
            <ArrowUp className="w-3 h-3 " />
          </div>
          <div className="border border-border p-1 rounded-lg bg-gray-200 dark:bg-zinc-950">
            <ArrowDown className="w-3 h-3 " />{" "}
          </div>
          <div className="text-[10px]">to navigate</div>
        </div>
        <div className="flex flex-row items-center gap-1 text-xs">
          <div className=" border border-border p-1 rounded-lg bg-gray-200 dark:bg-zinc-950">
            <CornerDownLeft className="w-3 h-3 " />{" "}
          </div>
          <div className="text-[10px]">to select</div>
        </div>
      </div>
      <div className="flex flex-row items-center gap-2">
        <Button
          variant="titleBar"
          onClick={() => setIsSettingsOpen(true)}
          size="sm"
          className="group flex flex-row items-center gap-1 z-10"
        >
          <Folder className="h-4 w-4" />
        </Button>
        <ThemeToggle />
      </div>
    </div>
  );
}
