import { useState, useEffect, useRef, KeyboardEvent } from "react";
import { Input } from "./components/ui/input";
import { invoke } from "@tauri-apps/api/core";
import { cn } from "./lib/utils";
import { AppSettings, ChatMessage } from "./types/types";
import { Button } from "./components/ui/button";
import { RxArrowTopRight } from "react-icons/rx";

interface Props {
  searchQuery: string;
  setSearchQuery: (val: string) => void;
  settings: AppSettings | null;
  setIsSettingsOpen: (val: boolean) => void;
}

export default function Header(props: Props) {
  const { searchQuery, setSearchQuery, settings, setIsSettingsOpen } = props;
  const inputRef = useRef<HTMLInputElement>(null);
  const [isRagMode, setIsRagMode] = useState(false);
  const [chatMessages, setChatMessages] = useState<ChatMessage[]>([]);
  const [isProcessing, setIsProcessing] = useState(false);
  const wrapperRef = useRef<HTMLDivElement>(null);
  const [showModelMissingPrompt, setShowModelMissingPrompt] =
    useState<boolean>(false);

  const doesModelExist = Boolean(settings?.selected_model_id);

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
      setShowModelMissingPrompt(false);
    }
  }, [searchQuery, isRagMode]);

  // Hide model missing prompt if settings update and model is selected
  useEffect(() => {
    if (doesModelExist) {
      setShowModelMissingPrompt(false);
    }
  }, [doesModelExist]);

  const handleKeyDown = async (e: KeyboardEvent<HTMLInputElement>) => {
    // If "/" is pressed as the first character to activate RAG mode
    if (e.key === "/" && searchQuery === "" && !isRagMode) {
      e.preventDefault();
      setIsRagMode(true);
      setSearchQuery("/");

      // Check if model exists when entering RAG mode
      if (!doesModelExist) {
        setShowModelMissingPrompt(true);
      }
    }

    // Handle backspace when only "/" is present
    if (searchQuery === "/" && e.key === "Backspace") {
      e.preventDefault();
      setIsRagMode(false);
      setSearchQuery("");
      setShowModelMissingPrompt(false);
    }

    // Handle submitting a RAG query with Enter - only allow if model exists
    if (e.key === "Enter" && isRagMode && searchQuery.length > 1) {
      e.preventDefault();

      // Don't process if no model is selected
      if (!doesModelExist) {
        // Ensure the model missing prompt is shown
        setShowModelMissingPrompt(true);
        return;
      }

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
      setShowModelMissingPrompt(false);
    }
  };

  const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const newValue = e.target.value;
    setSearchQuery(newValue);

    // If in RAG mode and user completely deletes the input, exit RAG mode
    if (isRagMode && newValue === "") {
      setIsRagMode(false);
      setShowModelMissingPrompt(false);
    }
  };

  const handleSelectModel = () => {
    if (setIsSettingsOpen) {
      setIsSettingsOpen(true);
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
          placeholder={
            isRagMode
              ? doesModelExist
                ? "Ask a question about your documents..."
                : "Please select a model first to ask questions"
              : "Type a command or search..."
          }
          value={searchQuery}
          autoCorrect="off"
          spellCheck="false"
          ref={inputRef}
          autoFocus
          onChange={handleChange}
          onKeyDown={handleKeyDown}
          className={cn(
            `text-xs placeholder:pl-2 border-0 focus-visible:outline-hidden focus-visible:ring-0 shadow-none dark:text-white text-gray-900`,
            !doesModelExist && isRagMode ? "text-gray-400" : ""
          )}
        />
      </div>

      {/* Model Missing Prompt */}
      {isRagMode && showModelMissingPrompt && !doesModelExist && (
        <div className="mt-4 px-2 flex justify-center">
          <Button
            className="text-xs text-gray-200 justify-between hover:cursor-pointer"
            onClick={handleSelectModel}
          >
            <span>No model detected, please select a model</span>
            <RxArrowTopRight />
          </Button>
        </div>
      )}

      {/* Chat Interface */}
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
