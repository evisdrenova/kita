import { RiExpandUpDownFill } from "react-icons/ri";

export default function WindowAction() {
  const handleClose = () => window.electron.closeWindow();
  const handleMinimize = () => window.electron.minimizeWindow();
  const handleMaximize = () => window.electron.maximizeWindow();
  return (
    <div
      className="flex items-center gap-2  no-drag group" // Added group class
    >
      <button
        onClick={handleClose}
        className="w-4 h-4 rounded-full bg-background text-primary-foreground/70 border-border border flex items-center justify-center text-xs no-drag"
        title="Close"
      >
        <span className="opacity-0 text-[9px] group-hover:opacity-100">✕</span>
      </button>
      <button
        onClick={handleMinimize}
        className="w-4 h-4 rounded-full bg-background text-primary-foreground/70 flex border-border border items-center justify-center text-gray-900 text-xs no-drag"
        title="Minimize"
      >
        <span className="opacity-0 group-hover:opacity-100">−</span>
      </button>
      <button
        onClick={handleMaximize}
        className="w-4 h-4 rounded-full bg-background text-primary-foreground/70 border-border border flex items-center justify-center text-gray-900 text-xs no-drag"
        title="Maximize"
      >
        <span className="opacity-0 group-hover:opacity-100 -rotate-45">
          <RiExpandUpDownFill />
        </span>
      </button>
    </div>
  );
}
