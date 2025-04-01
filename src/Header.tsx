import { useState, useEffect, useRef, KeyboardEvent } from "react";
import { Input } from "./components/ui/input";
import { invoke } from "@tauri-apps/api/core";
import { cn } from "./lib/utils";
import { ChatMessage } from "./types/types";

interface Props {
  searchQuery: string;
  setSearchQuery: (val: string) => void;
}

export default function Header(props: Props) {
  const { searchQuery, setSearchQuery } = props;
  const inputRef = useRef<HTMLInputElement>(null);
  const [isRagMode, setIsRagMode] = useState(false);
  const [chatMessages, setChatMessages] = useState<ChatMessage[]>([]);
  const [isProcessing, setIsProcessing] = useState(false);
  const wrapperRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  // handles making the window draggable by clicking on the area above the input
  useEffect(() => {
    if (wrapperRef.current) {
      wrapperRef.current.setAttribute("data-tauri-drag-region", "");
    }
  }, []);

  // This useEffect monitors searchQuery changes to detect when "/" is deleted
  useEffect(() => {
    if (isRagMode && searchQuery === "") {
      setIsRagMode(false);
    }
  }, [searchQuery, isRagMode]);

  const handleKeyDown = async (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "/" && searchQuery === "" && !isRagMode) {
      e.preventDefault();
      setIsRagMode(true);
      setSearchQuery("/");
    }

    if (searchQuery === "/" && e.key === "Backspace") {
      e.preventDefault();
      setIsRagMode(false);
      setSearchQuery("");
    }

    // Handle submitting a RAG query with Enter
    if (e.key === "Enter" && isRagMode && searchQuery.length > 1) {
      e.preventDefault();

      const userQuery = searchQuery.startsWith("/")
        ? searchQuery.substring(1)
        : searchQuery;
      setChatMessages((prev) => [
        ...prev,
        { role: "user", content: userQuery },
      ]);

      setSearchQuery("");

      setIsProcessing(true);
      try {
        const response = await invoke<string>("ask_llm", { prompt: userQuery });
        setChatMessages((prev) => [
          ...prev,
          { role: "assistant", content: response },
        ]);
      } catch (error) {
        console.error("Error processing RAG query:", error);
        setChatMessages((prev) => [
          ...prev,
          {
            role: "assistant",
            content: "Sorry, I encountered an error processing your request.",
          },
        ]);
      } finally {
        setIsProcessing(false);
        inputRef.current?.focus();
      }
    }

    if (e.key === "Escape" && isRagMode) {
      setIsRagMode(false);
      setSearchQuery("");
      setChatMessages([]);
    }
  };

  const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const newValue = e.target.value;
    setSearchQuery(newValue);

    if (isRagMode && newValue === "") {
      setIsRagMode(false);
    }
  };

  return (
    <div
      className="sticky top-0 flex flex-col gap-2 border-b border-b-border p-2"
      ref={wrapperRef}
      data-tauri-drag-region=""
    >
      <div
        className="flex flex-row items-center justify-between"
        data-tauri-drag-region=""
      >
        <Input
          placeholder={"Type a command or search..."}
          value={searchQuery}
          autoCorrect="off"
          spellCheck="false"
          ref={inputRef}
          autoFocus
          onChange={handleChange}
          onKeyDown={handleKeyDown}
          className={cn(
            `text-xs placeholder:pl-2 border-0 focus-visible:outline-hidden focus-visible:ring-0 shadow-none dark:text-white text-gray-900`
          )}
        />
      </div>
      {isRagMode && chatMessages.length > 0 && (
        <ChatInterface
          chatMessages={chatMessages}
          isProcessing={isProcessing}
        />
      )}
    </div>
  );
}

interface ChatInterfaceProps {
  chatMessages: ChatMessage[];
  isProcessing: boolean;
}

function ChatInterface(props: ChatInterfaceProps) {
  const { chatMessages, isProcessing } = props;

  return (
    <div className="mt-4 space-y-4 max-h-[60vh] overflow-y-auto">
      {chatMessages.map((message, index) => (
        <div
          key={index}
          className={`p-3 rounded-lg ${
            message.role === "user" ? "bg-primary/10 ml-8" : "bg-secondary mr-8"
          }`}
        >
          <div className="text-sm">{message.content}</div>
        </div>
      ))}

      {isProcessing && <ProcessingAnimation />}
    </div>
  );
}

function ProcessingAnimation() {
  return (
    <div className="p-3 rounded-lg bg-secondary mr-8">
      <div className="flex space-x-2">
        <div className="w-2 h-2 rounded-full bg-primary/50 animate-bounce"></div>
        <div
          className="w-2 h-2 rounded-full bg-primary/50 animate-bounce"
          style={{ animationDelay: "0.2s" }}
        ></div>
        <div
          className="w-2 h-2 rounded-full bg-primary/50 animate-bounce"
          style={{ animationDelay: "0.4s" }}
        ></div>
      </div>
    </div>
  );
}
