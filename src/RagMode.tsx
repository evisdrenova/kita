import { RxArrowTopRight } from "react-icons/rx";
import { Button } from "./components/ui/button";
import { modelStatus } from "./Header";
import { ChatMessage } from "./types/types";
import { cn } from "./lib/utils";
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface Props {
  isRagMode: boolean;
  showModelMissingPrompt: boolean;
  modelStatus: modelStatus;
  handleSelectModel: () => void;
  isCheckingModel: boolean;
  getModelErrorMessage: () => string;
  chatMessages: ChatMessage[];
  isProcessing: boolean;
}

export default function RagMode(props: Props) {
  const {
    isRagMode,
    showModelMissingPrompt,
    modelStatus,
    handleSelectModel,
    isCheckingModel,
    getModelErrorMessage,
    chatMessages,
    isProcessing,
  } = props;

  return (
    <>
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
    </>
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
