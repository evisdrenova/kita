import { useState, useEffect, useRef, KeyboardEvent } from "react";
import { Input } from "./components/ui/input";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { cn } from "./lib/utils";
import {
  AppSettings,
  ChatMessage,
  CompletionResponse,
  Contact,
  Model,
} from "./types/types";
import { Button } from "./components/ui/button";
import { RxArrowTopRight } from "react-icons/rx";

interface Props {
  searchQuery: string;
  setSearchQuery: (val: string) => void;
  settings: AppSettings | null;
  setIsSettingsOpen: (val: boolean) => void;
  contactData: Contact[];
}

const COMMANDS = ["/text", "/email", "/call", "/search", "/find"];

type modelStatus = "none" | "not-downloaded" | "ready";

type autocompleteType = "tools" | "people" | null;

export default function Header(props: Props) {
  const {
    searchQuery,
    setSearchQuery,
    settings,
    setIsSettingsOpen,
    contactData,
  } = props;
  const inputRef = useRef<HTMLInputElement>(null);
  const [isRagMode, setIsRagMode] = useState<boolean>(false);
  const [chatMessages, setChatMessages] = useState<ChatMessage[]>([]);
  const [isProcessing, setIsProcessing] = useState<boolean>(false);
  const [showModelMissingPrompt, setShowModelMissingPrompt] =
    useState<boolean>(false);
  const [modelStatus, setModelStatus] = useState<modelStatus>("none");
  const [availableModels, setAvailableModels] = useState<Model[]>([]);
  const [isCheckingModel, setIsCheckingModel] = useState<boolean>(false);

  // Autocomplete related states
  const [showAutocomplete, setShowAutocomplete] = useState<boolean>(false);
  const [autocompleteItems, setAutocompleteItems] = useState<string[]>([]);
  const [autocompleteType, setAutocompleteType] =
    useState<autocompleteType>(null);
  const [selectedAutocompleteIndex, setSelectedAutocompleteIndex] =
    useState<number>(0);
  const [cursorPosition, setCursorPosition] = useState<number>(0);
  const [_, setCurrentWord] = useState<string>("");

  // Track the previous selected model ID to detect changes
  const previousModelIdRef = useRef<string | null>(null);

  // Use a ref to track if we're clearing the input due to submission
  // This helps prevent turning off RAG mode when submitting
  const isSubmitting = useRef(false);

  // sets focus to the input
  useEffect(() => {
    inputRef.current?.focus();
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

  // Monitor input for autocomplete triggers
  useEffect(() => {
    if (!searchQuery) {
      setShowAutocomplete(false);
      return;
    }

    const cursorPos = inputRef.current?.selectionStart || searchQuery.length;
    setCursorPosition(cursorPos);

    // Get the current word being typed
    const textBeforeCursor = searchQuery.slice(0, cursorPos);
    const wordsBeforeCursor = textBeforeCursor.split(/\s+/);
    const currentWordBeingTyped =
      wordsBeforeCursor[wordsBeforeCursor.length - 1];
    setCurrentWord(currentWordBeingTyped);

    // Check for slash command autocomplete at the beginning of input
    if (
      currentWordBeingTyped.startsWith("/") &&
      wordsBeforeCursor.length === 1
    ) {
      const query = currentWordBeingTyped.slice(1).toLowerCase();
      const filteredCommands = COMMANDS.filter((command) =>
        command.toLowerCase().includes(query)
      );

      if (filteredCommands.length > 0) {
        setAutocompleteItems(filteredCommands);
        setAutocompleteType("tools");
        setShowAutocomplete(true);
        setSelectedAutocompleteIndex(0);
        return;
      }
    }

    // Check for contact autocomplete with @ symbol
    if (
      currentWordBeingTyped.startsWith("@") &&
      currentWordBeingTyped.length >= 2
    ) {
      const query = currentWordBeingTyped.slice(1).toLowerCase();
      const filteredContacts = contactData.filter((contact) => {
        const name =
          `${contact.given_name} ${contact.family_name}`.toLowerCase();
        return name.includes(query);
      });

      if (filteredContacts.length > 0) {
        // Create display strings for contacts
        const contactItems = filteredContacts.map(
          (contact) => `@${contact.given_name} ${contact.family_name}`
        );

        setAutocompleteItems(contactItems);
        setAutocompleteType("people");
        setShowAutocomplete(true);
        setSelectedAutocompleteIndex(0);
        return;
      }
    }

    // If no matches or not triggering condition
    setShowAutocomplete(false);
  }, [searchQuery, contactData]);

  // Function to apply the selected autocomplete item
  const applyAutocomplete = (item: string) => {
    if (!inputRef.current) return;

    const beforeCursor = searchQuery.slice(0, cursorPosition);
    const afterCursor = searchQuery.slice(cursorPosition);

    // Find the start position of the current word
    const wordStartPos = beforeCursor.lastIndexOf(" ") + 1;

    // Replace the current word with the selected item
    const newQuery =
      beforeCursor.slice(0, wordStartPos) +
      item +
      (autocompleteType === "people" ? " " : "") + // Add space after people names
      afterCursor;

    setSearchQuery(newQuery);
    setShowAutocomplete(false);

    // Set cursor position after the inserted word (and space for people)
    const newCursorPos =
      wordStartPos + item.length + (autocompleteType === "people" ? 1 : 0);

    // Need to wait for the input to update before setting the cursor position
    setTimeout(() => {
      if (inputRef.current) {
        inputRef.current.focus();
        inputRef.current.setSelectionRange(newCursorPos, newCursorPos);
      }
    }, 0);
  };

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

  // Update dropdown position when input changes
  useEffect(() => {
    if (showAutocomplete && inputRef.current) {
      // Force a re-render to update the dropdown position
      setShowAutocomplete(false);
      setTimeout(() => {
        setShowAutocomplete(true);
      }, 0);
    }
  }, [
    inputRef.current?.getBoundingClientRect().width,
    inputRef.current?.getBoundingClientRect().height,
  ]);

  // Close autocomplete when clicking outside
  useEffect(() => {
    function handleClickOutside(event: MouseEvent) {
      if (
        inputRef.current &&
        !inputRef.current.contains(event.target as Node) &&
        showAutocomplete
      ) {
        setShowAutocomplete(false);
      }
    }

    document.addEventListener("mousedown", handleClickOutside);
    return () => {
      document.removeEventListener("mousedown", handleClickOutside);
    };
  }, [showAutocomplete]);

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

  const submitMessage = async () => {
    try {
      // Make a backend call to submit the message
      await invoke("submit_message", {
        message: searchQuery,
      });

      // Add message to chat history
      setChatMessages((prev) => [
        ...prev,
        { role: "user", content: searchQuery },
      ]);

      // Clear input after successful submission
      setSearchQuery("");
      console.log("Message submitted successfully:", searchQuery);
    } catch (error) {
      console.error("Error submitting message:", error);
    }
  };

  const handleKeyDown = async (e: KeyboardEvent<HTMLInputElement>) => {
    // Handle autocomplete navigation
    if (showAutocomplete) {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setSelectedAutocompleteIndex(
          (prevIndex) => (prevIndex + 1) % autocompleteItems.length
        );
        return;
      }

      if (e.key === "ArrowUp") {
        e.preventDefault();
        setSelectedAutocompleteIndex(
          (prevIndex) =>
            (prevIndex - 1 + autocompleteItems.length) %
            autocompleteItems.length
        );
        return;
      }

      if (e.key === "Tab") {
        e.preventDefault();
        applyAutocomplete(autocompleteItems[selectedAutocompleteIndex]);
        return;
      }

      if (e.key === "Enter") {
        e.preventDefault();

        // If we have exactly one filtered item, apply it and continue with submission if it's a contact
        if (autocompleteItems.length === 1) {
          applyAutocomplete(autocompleteItems[0]);

          // If this was a contact being selected, move to the next part
          if (autocompleteType === "people") {
            // Set a small timeout to allow the input to update before potentially submitting
            setTimeout(() => {
              if (inputRef.current) {
                inputRef.current.focus();
              }
            }, 10);
          }
          return;
        } else {
          // If we have multiple items, just apply the selected one
          applyAutocomplete(autocompleteItems[selectedAutocompleteIndex]);
          return;
        }
      }

      if (e.key === "Escape") {
        e.preventDefault();
        setShowAutocomplete(false);
        return;
      }
    }

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

    // Handle Enter to submit
    if (e.key === "Enter" && searchQuery.trim() !== "" && !showAutocomplete) {
      e.preventDefault();

      // For RAG mode
      if (isRagMode && searchQuery.length > 1) {
        // RAG mode handling (unchanged)
        await checkModelStatus();

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

        isSubmitting.current = true;
        setSearchQuery(">");
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
      } else {
        // Process slash commands
        if (searchQuery.startsWith("/")) {
          const parts = searchQuery.split(" ");
          const command = parts[0].toLowerCase();

          // Extract command details for backend
          let commandType = "";
          let recipient = "";
          let message = "";

          if (
            command === "/text" ||
            command === "/email" ||
            command === "/call"
          ) {
            commandType = command.substring(1); // Remove the slash

            // Find recipient (marked with @)
            const recipientIndex = parts.findIndex((part) =>
              part.startsWith("@")
            );

            if (recipientIndex > 0) {
              recipient = parts[recipientIndex].substring(1); // Remove the @

              // Everything after the recipient is the message
              message = parts.slice(recipientIndex + 1).join(" ");
            } else {
              // No recipient specified, use rest as message
              message = parts.slice(1).join(" ");
            }
          } else if (command === "/search" || command === "/find") {
            commandType = "search";
            message = parts.slice(1).join(" ");
          }

          // Send the command to backend
          try {
            await invoke("handle_command", {
              commandType,
              recipient,
              message,
            });

            setChatMessages((prev) => [
              ...prev,
              { role: "user", content: searchQuery },
            ]);

            setSearchQuery("");
          } catch (error) {
            console.error("Error processing command:", error);
          }
        } else {
          // Normal message submission
          await submitMessage();
        }
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
      data-tauri-drag-region=""
    >
      <div className="flex flex-col">
        <div
          className="flex flex-row items-center justify-between relative"
          data-tauri-drag-region=""
        >
          <Input
            placeholder={
              isRagMode
                ? modelStatus === "ready"
                  ? "Ask a question about your documents..."
                  : "Please select or download a model first"
                : "Type / for commands (e.g. /text @contact message)"
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

        {/* Autocomplete dropdown */}
        {showAutocomplete && autocompleteItems.length > 0 && (
          <div className="fixed mt-8 z-[100] text-gray-800 dark:text-gray-100 bg-background border border-border rounded-md shadow-lg max-h-60 overflow-y-auto">
            <ul className="py-1 z-50">
              {autocompleteItems.map((item, index) => (
                <li
                  key={item}
                  className={cn(
                    "px-4 py-2 text-sm cursor-pointer hover:bg-accent",
                    index === selectedAutocompleteIndex ? "bg-accent" : ""
                  )}
                  onClick={() => applyAutocomplete(item)}
                >
                  {item}
                </li>
              ))}
            </ul>
          </div>
        )}
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
