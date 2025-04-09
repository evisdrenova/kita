import { useState, useEffect, useRef, KeyboardEvent } from "react";
import { Input } from "./components/ui/input";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { cn } from "./lib/utils";
import {
  AppSettings,
  ChatMessage,
  CompletionResponse,
  Model,
} from "./types/types";
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
  const [modelStatus, setModelStatus] = useState<
    "none" | "not-downloaded" | "ready"
  >("none");
  const [availableModels, setAvailableModels] = useState<Model[]>([]);
  const [isCheckingModel, setIsCheckingModel] = useState(false);

  // Track the previous selected model ID to detect changes
  const previousModelIdRef = useRef<string | null>(null);

  // Use a ref to track if we're clearing the input due to submission
  // This helps prevent turning off RAG mode when submitting
  const isSubmitting = useRef(false);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  // handles making the window draggable by clicking on the area above the input
  useEffect(() => {
    if (wrapperRef.current) {
      wrapperRef.current.setAttribute("data-tauri-drag-region", "");
    }
  }, []);

  // Check model status whenever settings change (especially selected_model_id)
  useEffect(() => {
    // Skip if settings aren't loaded yet
    if (!settings) return;

    const currentModelId = settings.selected_model_id || null;
    const modelChanged = previousModelIdRef.current !== currentModelId;

    // Update the ref for future change detection
    previousModelIdRef.current = currentModelId;

    // If in RAG mode or the model has changed, check model status
    if (isRagMode || modelChanged) {
      checkModelStatus();
    }
  }, [settings, isRagMode]);

  // Function to check model status - pulled out to reuse
  const checkModelStatus = async () => {
    if (!settings) return;

    // No model selected
    if (!settings.selected_model_id) {
      setModelStatus("none");
      setShowModelMissingPrompt(true);
      return;
    }

    try {
      setIsCheckingModel(true);

      // Get all available models
      const models = await invoke<Model[]>("get_models", {
        customPath: settings.custom_model_path || null,
      });
      setAvailableModels(models);

      // Find the selected model
      const selectedModel = models.find(
        (m) => m.id === settings.selected_model_id
      );

      if (!selectedModel) {
        setModelStatus("none");
        setShowModelMissingPrompt(true);
        console.error("Selected model not found in available models");
        return;
      }

      // Check if selected model is downloaded
      if (!selectedModel.is_downloaded) {
        setModelStatus("not-downloaded");
        setShowModelMissingPrompt(true);
      } else {
        setModelStatus("ready");
        setShowModelMissingPrompt(false);
      }
    } catch (error) {
      console.error("Failed to check model status:", error);
      setModelStatus("none");
      setShowModelMissingPrompt(true);
    } finally {
      setIsCheckingModel(false);
    }
  };

  // Listen for settings panel close to refresh model status
  useEffect(() => {
    let isMounted = true;

    // Create a function that checks model status when settings are closed
    const handleSettingsClose = async () => {
      // Small delay to ensure settings are updated
      await new Promise((resolve) => setTimeout(resolve, 100));
      if (isMounted && isRagMode) {
        await checkModelStatus();
      }
    };

    // Use MutationObserver to detect when settings panel is removed from DOM
    const observer = new MutationObserver((mutations) => {
      mutations.forEach((mutation) => {
        if (mutation.type === "childList" && mutation.removedNodes.length > 0) {
          // Check if any removed node could be the settings panel
          const removedSettings = Array.from(mutation.removedNodes).some(
            (node) => {
              return (
                node instanceof HTMLElement &&
                (node.classList.contains("settings") ||
                  node.querySelector(".settings"))
              );
            }
          );

          if (removedSettings) {
            handleSettingsClose();
          }
        }
      });
    });

    // Start observing the document body
    observer.observe(document.body, { childList: true, subtree: true });

    return () => {
      isMounted = false;
      observer.disconnect();
    };
  }, [isRagMode]);

  // This useEffect monitors searchQuery changes to detect when ">" is deleted
  useEffect(() => {
    // Only turn off RAG mode if it's empty and we're not in the process of submitting
    if (isRagMode && searchQuery === "" && !isSubmitting.current) {
      setIsRagMode(false);
      setShowModelMissingPrompt(false);
    }

    // Reset the submitting flag once searchQuery has been updated
    if (isSubmitting.current) {
      isSubmitting.current = false;
    }
  }, [searchQuery, isRagMode]);

  // Handle model events and monitor model selection changes
  useEffect(() => {
    // Listen for model selection required event
    const unlistenSelectionPromise = listen("model-selection-required", () => {
      console.log("Model selection required event received");
      setModelStatus("none");
      setShowModelMissingPrompt(true);
      // Open settings dialog if we're in RAG mode
      if (isRagMode) {
        setIsSettingsOpen(true);
      }
    });

    // Listen for model download required event
    const unlistenDownloadPromise = listen(
      "model-download-required",
      (event) => {
        console.log("Model download required:", event.payload);
        setModelStatus("not-downloaded");
        setShowModelMissingPrompt(true);
        // If we're in RAG mode, open settings to allow download
        if (isRagMode) {
          setIsSettingsOpen(true);
        }
      }
    );

    // Listen for model updates
    const unlistenModelUpdatePromise = listen("model-updated", () => {
      console.log("Model update event received, rechecking status");
      checkModelStatus();
    });

    // Clean up listeners on component unmount
    return () => {
      unlistenSelectionPromise.then((unlistenFn) => unlistenFn());
      unlistenDownloadPromise.then((unlistenFn) => unlistenFn());
      unlistenModelUpdatePromise.then((unlistenFn) => unlistenFn());
    };
  }, [isRagMode, setIsSettingsOpen]);

  const handleKeyDown = async (e: KeyboardEvent<HTMLInputElement>) => {
    // If ">" is pressed as the first character to activate RAG mode
    if (e.key === ">" && searchQuery === "" && !isRagMode) {
      e.preventDefault();
      setIsRagMode(true);
      setSearchQuery(">");

      // Check model status when entering RAG mode
      checkModelStatus();
    }

    // Handle backspace when only ">" is present
    if (searchQuery === ">" && e.key === "Backspace") {
      e.preventDefault();
      setIsRagMode(false);
      setSearchQuery("");
      setShowModelMissingPrompt(false);
    }

    // Handle submitting a RAG query with Enter - only allow if model exists and is downloaded
    if (e.key === "Enter" && isRagMode && searchQuery.length > 1) {
      e.preventDefault();

      // First check model status to be sure it's up to date
      await checkModelStatus();

      // Don't process if model status isn't ready
      if (modelStatus !== "ready") {
        setShowModelMissingPrompt(true);
        return;
      }

      const userQuery = searchQuery.startsWith(">")
        ? searchQuery.substring(1)
        : searchQuery;
      setChatMessages((prev) => [
        ...prev,
        { role: "user", content: userQuery },
      ]);

      // Set the submitting flag before clearing search query
      isSubmitting.current = true;
      setSearchQuery(">"); // Keep the ">" to maintain RAG mode
      setIsProcessing(true);

      try {
        const response = await invoke<CompletionResponse>("ask_llm", {
          prompt: userQuery,
        });

        setChatMessages((prev) => [
          ...prev,
          {
            role: "assistant",
            content: response.content,
            sources: response.sources,
          },
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
    // But don't exit if we're submitting
    if (isRagMode && newValue === "" && !isSubmitting.current) {
      setIsRagMode(false);
      setShowModelMissingPrompt(false);
    }
  };

  const handleSelectModel = () => {
    if (setIsSettingsOpen) {
      setIsSettingsOpen(true);
    }
  };

  const getModelErrorMessage = () => {
    if (modelStatus === "none") {
      return "No model selected, please select a model";
    } else if (modelStatus === "not-downloaded") {
      const selectedModelName =
        availableModels.find((m) => m.id === settings?.selected_model_id)
          ?.name || "selected model";
      return `${selectedModelName} needs to be downloaded`;
    }
    return "";
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
              ? modelStatus === "ready"
                ? "Ask a question about your documents..."
                : "Please select or download a model first"
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
            modelStatus !== "ready" && isRagMode ? "text-gray-400" : ""
          )}
        />
      </div>
      {isRagMode && showModelMissingPrompt && modelStatus !== "ready" && (
        <MissingModel
          handleSelectModel={handleSelectModel}
          isCheckingModel={isCheckingModel}
          getModelErrorMessage={getModelErrorMessage}
        />
      )}
      {isRagMode && chatMessages.length > 0 && (
        <ChatInterface
          chatMessages={chatMessages}
          isProcessing={isProcessing}
        />
      )}
    </div>
  );
}

interface MissingModelProps {
  handleSelectModel: () => void;
  isCheckingModel: boolean;
  getModelErrorMessage: () => string;
}

function MissingModel(props: MissingModelProps) {
  const { handleSelectModel, isCheckingModel, getModelErrorMessage } = props;
  return (
    <div className="mt-4 px-2 flex justify-center">
      <Button
        className="text-xs text-gray-200 justify-between hover:cursor-pointer"
        onClick={handleSelectModel}
        disabled={isCheckingModel}
      >
        {isCheckingModel ? (
          <span>Checking model status...</span>
        ) : (
          <>
            <span>{getModelErrorMessage()}</span>
            <RxArrowTopRight />
          </>
        )}
      </Button>
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
          className={cn(
            message.role === "user" ? "justify-end" : "justify-start ",
            `flex `
          )}
        >
          <div className="flex flex-col max-w-[60%]">
            <div
              className={cn(
                message.role == "user" ? "justify-end" : "justify-start",
                "flex flex-row gap-1"
              )}
            >
              <div
                className={cn(
                  message.role === "user"
                    ? "bg-background justify-end"
                    : "bg-primary",
                  `rounded-lg px-4 py-2 text-primary-foreground  border border-border`
                )}
              >
                <div className="text-sm">{message.content}</div>
                {message.sources && message.sources.length > 0 && (
                  <div className="mt-2 pt-2 border-t border-primary-foreground/20">
                    <div className="text-xs text-primary-foreground/70 mb-1">
                      Sources:
                    </div>
                    <div className="flex flex-col flex-wrap gap-2">
                      {message.sources.map((source, idx) => (
                        <div className="flex flex-row items-center gap-2 cursor-pointer">
                          <SourceBadge key={idx} source={source} idx={idx} />
                        </div>
                      ))}
                    </div>
                  </div>
                )}
              </div>
            </div>
          </div>
        </div>
      ))}

      {isProcessing && <ProcessingAnimation />}
    </div>
  );
}

// Add a new SourceBadge component
interface SourceBadgeProps {
  source: string;
  idx: number;
}

function SourceBadge(props: SourceBadgeProps) {
  const { source, idx } = props;
  const [fileName, setFileName] = useState<string | null>(null);

  // Fetch file name from file ID
  useEffect(() => {
    async function fetchFileName() {
      try {
        const fileInfo = await invoke<{ name: string }>("get_file_by_id", {
          fileId: source,
        });
        setFileName(fileInfo.name);
      } catch (error) {
        console.error("Failed to fetch file name:", error);
      }
    }

    fetchFileName();
  }, [source]);

  const handleClick = async () => {
    try {
      await invoke("open_file_by_id", { fileId: source });
    } catch (error) {
      console.error("Failed to open file:", error);
    }
  };

  return (
    <button
      onClick={handleClick}
      className="inline-flex items-center px-2 py-1 rounded-md text-xs bg-primary-foreground/20 hover:bg-primary-foreground/30 transition-colors gap-2"
    >
      <div>[{idx + 1}]</div> <span>{fileName || `${source}`}</span>
    </button>
  );
}

function ProcessingAnimation() {
  return (
    <div className="p-3 rounded-lg bg-secondary mr-8 w-[60%]">
      <div className="flex space-x-2">
        <div className="w-2 h-2 rounded-full bg-gray-700 animate-bounce"></div>
        <div
          className="w-2 h-2 rounded-full bg-gray-700 animate-bounce"
          style={{ animationDelay: "0.2s" }}
        ></div>
        <div
          className="w-2 h-2 rounded-full bg-gray-700 animate-bounce"
          style={{ animationDelay: "0.4s" }}
        ></div>
      </div>
    </div>
  );
}
