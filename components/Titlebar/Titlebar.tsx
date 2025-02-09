import { RiExpandUpDownFill } from "react-icons/ri";
import { IoSearchOutline } from "react-icons/io5";
import { useState, useEffect } from "react";

import {
  CommandDialog,
  CommandEmpty,
  CommandInput,
  CommandList,
} from "../../components/ui/command";
import { VisuallyHidden } from "@radix-ui/react-visually-hidden";
import { ThemeToggle } from "../../src/ThemeProvider";
import { Button } from "../../components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogTitle,
  DialogTrigger,
} from "../../components/ui/dialog";
import Settings from "../../src/pages/Settings";
import { Settings2 } from "lucide-react";
import ChatTitle from "../../components/ChatInterface/ChatTitle";
import { Conversation } from "../../src/types";

interface Props {
  activeConversation: Conversation;
  onUpdateTitle: (convoId: number, newTitle: string) => void;
  onDeleteConversation: (convoId: number) => void;
}

export default function TitleBar(props: Props) {
  const { activeConversation, onUpdateTitle, onDeleteConversation } = props;
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
      <div className="no-drag">
        <ChatTitle
          title={activeConversation?.title}
          id={activeConversation?.id}
          onDeleteConversation={onDeleteConversation}
          onUpdateTitle={onUpdateTitle}
        />
      </div>
      <div className="flex flex-row items-center rounded-lg no-drag ">
        <Button
          variant="titleBar"
          onClick={(e) => handleSearch(e)}
          size="sm"
          className="group flex flex-row items-center gap-1 z-10 "
        >
          <IoSearchOutline size={14} />
          <p className="text-sm">
            <kbd className="inline-flex h-5 select-none items-center gap-1 rounded bg-muted group-hover:bg-text-foreground px-1.5 font-mono text-[10px] font-medium opacity-100">
              <span className="text-lg">⌘</span>S
            </kbd>
          </p>
        </Button>
        <ThemeToggle />
        <Dialog>
          <DialogTrigger>
            <Settings2
              size={16}
              className="text-primary-foreground/70 hover:text-primary-foreground"
            />
          </DialogTrigger>
          <DialogContent>
            <Settings />
          </DialogContent>
        </Dialog>
      </div>
      <CommandDialogComponent open={open} setOpen={setOpen} />
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

interface CommandProps {
  open: boolean;
  setOpen: (val: boolean) => void;
}

function CommandDialogComponent(props: CommandProps) {
  const { open, setOpen } = props;
  return (
    <CommandDialog open={open} onOpenChange={setOpen}>
      <VisuallyHidden>
        <DialogTitle />
      </VisuallyHidden>
      <CommandInput
        placeholder="Type a command or search..."
        className="text-xs"
      />
      <CommandList>
        <CommandEmpty className="text-xs text-main-900 p-4">
          No results found.
        </CommandEmpty>
        {/* <CommandGroup heading="Suggestions">
            <CommandItem>
              <Calendar />
              <span>Calendar</span>
            </CommandItem>
            <CommandItem>
              <Smile />
              <span>Search Emoji</span>
            </CommandItem>
            <CommandItem>
              <Calculator />
              <span>Calculator</span>
            </CommandItem>
          </CommandGroup>
          <CommandSeparator />
          <CommandGroup heading="Settings">
            <CommandItem>
              <User />
              <span>Profile</span>
              <CommandShortcut>⌘P</CommandShortcut>
            </CommandItem>
            <CommandItem>
              <CreditCard />
              <span>Billing</span>
              <CommandShortcut>⌘B</CommandShortcut>
            </CommandItem>
            <CommandItem>
              <Settings />
              <span>Settings</span>
              <CommandShortcut>⌘S</CommandShortcut>
            </CommandItem>
          </CommandGroup> */}
      </CommandList>
    </CommandDialog>
  );
}
