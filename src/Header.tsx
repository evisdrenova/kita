import { useState, useEffect, useRef, KeyboardEvent } from "react";
import { Input } from "./components/ui/input";
import { invoke } from "@tauri-apps/api/core";
import { cn } from "./lib/utils";

interface ChatMessage {
  role: "user" | "assistant";
  content: string;
}

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

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const handleKeyDown = async (e: KeyboardEvent<HTMLInputElement>) => {
    // If "/" is pressed as the first character and RAG mode is not active
    if (e.key === "/" && searchQuery === "" && !isRagMode) {
      e.preventDefault();
      setIsRagMode(true);
      setSearchQuery("/");
    }

    // Handle submitting a RAG query with Enter
    if (e.key === "Enter" && isRagMode && searchQuery.length > 1) {
      e.preventDefault();

      // Add user message to chat
      const userQuery = searchQuery.startsWith("/")
        ? searchQuery.substring(1)
        : searchQuery;
      setChatMessages((prev) => [
        ...prev,
        { role: "user", content: userQuery },
      ]);

      // Clear input after submission
      setSearchQuery("");

      // Process the RAG query
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
        // Focus the input again for the next query
        inputRef.current?.focus();
      }
    }

    if (e.key === "Escape" && isRagMode) {
      setIsRagMode(false);
      setSearchQuery("");
      setChatMessages([]);
    }
  };

  return (
    <div className="sticky top-0 flex flex-col gap-2 border-b border-b-border p-2">
      <div className="flex flex-row items-center justify-between">
        <Input
          placeholder={
            isRagMode
              ? "Ask a question about your documents..."
              : "Type a command or search..."
          }
          value={searchQuery}
          autoCorrect="off"
          spellCheck="false"
          ref={inputRef}
          autoFocus
          onChange={(e) => setSearchQuery(e.target.value)}
          onKeyDown={handleKeyDown}
          className={cn(
            `text-xs placeholder:pl-2 border-0 focus-visible:outline-hidden focus-visible:ring-0 shadow-none dark:text-white text-gray-900`
          )}
        />
      </div>
      {isRagMode && chatMessages.length > 0 && (
        <div className="mt-4 space-y-4 max-h-[60vh] overflow-y-auto">
          {chatMessages.map((message, index) => (
            <div
              key={index}
              className={`p-3 rounded-lg ${
                message.role === "user"
                  ? "bg-primary/10 ml-8"
                  : "bg-secondary mr-8"
              }`}
            >
              <div className="text-sm">{message.content}</div>
            </div>
          ))}

          {isProcessing && (
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
          )}
        </div>
      )}
    </div>
  );
}
