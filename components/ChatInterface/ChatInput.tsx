import {
  ArrowUp,
  Database,
  File,
  FileQuestion,
  FileText,
  Paperclip,
  Table,
  Wrench,
  X,
} from "lucide-react";
import { Textarea } from "../ui/textarea";
import { Button } from "../ui/button";
import { FileAttachment, Provider } from "../../src/types";
import ModelSelect from "./ModelSelect";
import { useRef, useState } from "react";
import { Dialog, DialogContent, DialogTrigger } from "../ui/dialog";
import { cn } from "../../src/lib/utils";
import { toast } from "sonner";

interface Props {
  inputValue: string;
  setInputValue: (val: string) => void;
  currentProvider: Provider;
  isLoading: boolean;
  handleSubmit: (
    e: React.FormEvent,
    attachments?: FileAttachment[]
  ) => Promise<void>;
  handleProviderSelect: (providerId: string) => void;
  selectedProvider: string;
  providers: Provider[];
  setOpenTools: (val: boolean) => void;
}

export default function ChatInput(props: Props) {
  const {
    handleSubmit,
    inputValue,
    setInputValue,
    currentProvider,
    isLoading,
    handleProviderSelect,
    selectedProvider,
    providers,
    setOpenTools,
  } = props;

  const [attachments, setAttachments] = useState<FileAttachment[]>([]);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [isDragging, setIsDragging] = useState(false);
  const [isProcessing, setIsProcessing] = useState(false);
  const [dragCounter, setDragCounter] = useState(0);

  const handleDragEnter = (e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setDragCounter((prev) => prev + 1);
    if (!isDragging) setIsDragging(true);
  };

  const handleDragLeave = (e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setDragCounter((prev) => prev - 1);
    if (dragCounter === 1) {
      setIsDragging(false);
    }
  };

  const handleDragOver = (e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
  };

  const handleDrop = async (e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setIsDragging(false);
    setIsProcessing(true);

    const files = Array.from(e.dataTransfer.files);
    const newAttachments: FileAttachment[] = [];

    try {
      for (const file of files) {
        const type = await detectFileType(file);
        const preview = await createFilePreview(file);
        newAttachments.push({
          id: crypto.randomUUID(),
          file,
          type,
          preview,
        });
      }

      setAttachments((current) => [...current, ...newAttachments]);
    } catch (error) {
      toast.error("Error processing dropped files:", error);
    } finally {
      setIsProcessing(false);
    }
  };

  const detectFileType = async (file: File): Promise<string> => {
    // Basic file type detection
    if (file.type.startsWith("image/")) return "image";
    if (file.type === "application/pdf") return "pdf";
    if (file.type === "text/csv") return "csv";
    if (file.type.includes("spreadsheet") || file.name.endsWith(".xlsx"))
      return "spreadsheet";
    if (file.type.startsWith("text/")) return "text";
    return "unknown";
  };

  const createFilePreview = async (file: File): Promise<string | undefined> => {
    if (file.type.startsWith("image/")) {
      return new Promise((resolve) => {
        const reader = new FileReader();
        reader.onloadend = () => resolve(reader.result as string);
        reader.readAsDataURL(file);
      });
    }
    return undefined;
  };

  const handleFileSelect = async () => {
    if (fileInputRef.current) {
      fileInputRef.current.click();
    }
  };

  const handleFileChange = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const files = Array.from(e.target.files || []);
    const newAttachments: FileAttachment[] = [];

    for (const file of files) {
      const type = await detectFileType(file);
      const preview = await createFilePreview(file);
      newAttachments.push({
        id: crypto.randomUUID(), // Add unique ID
        file,
        type,
        preview,
      });
    }

    setAttachments([...attachments, ...newAttachments]);
  };

  const removeAttachment = (id: string) => {
    const attachmentIndex = attachments.findIndex((a) => a.id === id);
    if (attachmentIndex === -1) return;

    const newAttachments = [
      ...attachments.slice(0, attachmentIndex),
      ...attachments.slice(attachmentIndex + 1),
    ];

    setAttachments(newAttachments);
  };

  const handlePaste = async (e: React.ClipboardEvent) => {
    const items = e.clipboardData.items;
    const newAttachments: FileAttachment[] = [];

    for (const item of Array.from(items)) {
      if (item.type.startsWith("image/")) {
        const file = item.getAsFile();
        if (file) {
          const type = await detectFileType(file);
          const preview = await createFilePreview(file);
          newAttachments.push({
            id: crypto.randomUUID(),
            file,
            type,
            preview,
          });
        }
      }
    }

    if (newAttachments.length > 0) {
      setAttachments((current) => [...current, ...newAttachments]);
    }
  };

  const onSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    await handleSubmit(e, attachments);
    setAttachments([]);
  };

  return (
    <div className="flex flex-col h-full">
      <div className="flex flex-col h-full flex-1 bg-muted border border-border rounded-lg mx-1 my-1">
        <ChatInputToolBar
          handleProviderSelect={handleProviderSelect}
          selectedProvider={selectedProvider}
          providers={providers}
          setOpenTools={setOpenTools}
          onAttachClick={handleFileSelect}
        />
        <div className="p-2 flex flex-col flex-1">
          <form onSubmit={onSubmit} className="flex flex-col flex-1">
            <div
              className={cn(
                "relative flex flex-row flex-1 gap-2 transition-all duration-200",
                isDragging && "ring-2 ring-primary rounded-lg"
              )}
              onDragEnter={handleDragEnter}
              onDragLeave={handleDragLeave}
              onDragOver={handleDragOver}
              onDrop={(e) => {
                handleDrop(e);
                setDragCounter(0);
                setIsDragging(false);
              }}
            >
              {isDragging && (
                <div className="absolute inset-0 bg-primary/10 rounded-lg flex items-center justify-center pointer-events-none">
                  <div className="bg-background/90 px-4 py-2 rounded-md shadow-lg">
                    Drop files here
                  </div>
                </div>
              )}
              {isProcessing && (
                <div className="absolute inset-0 bg-background/50 rounded-lg flex items-center justify-center">
                  <div className="bg-background px-4 py-2 rounded-md shadow-lg flex items-center gap-2">
                    <div className="animate-spin rounded-full h-4 w-4 border-2 border-primary border-t-transparent" />
                    Processing files...
                  </div>
                </div>
              )}
              <Textarea
                value={inputValue}
                onChange={(e) => setInputValue(e.target.value)}
                onPaste={handlePaste}
                placeholder="What are you working on?"
                className="placeholder:text-primary-foreground/40 placeholder:text-xs text-primary-foreground w-full resize-none p-3 !text-[12px] border-0 focus:ring-0 focus-visible:ring-0 shadow-none"
                onKeyDown={(e) => {
                  if (e.key === "Enter" && !e.shiftKey) {
                    e.preventDefault();
                    onSubmit(e);
                  }
                }}
              />
              <Button
                type="submit"
                size="sm"
                disabled={!inputValue || !currentProvider || isLoading}
                className="rounded-lg p-2 h-fit"
              >
                <ArrowUp className="w-4 h-4" />
              </Button>
            </div>
            {attachments.length > 0 && (
              <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 mt-2">
                {attachments.map((attachment) => (
                  <div className="p-1" key={attachment.id}>
                    <AttachmentPreview
                      attachment={attachment}
                      onRemove={() => removeAttachment(attachment.id)}
                    />
                  </div>
                ))}
              </div>
            )}
          </form>
          <input
            type="file"
            ref={fileInputRef}
            onChange={handleFileChange}
            className="hidden"
            multiple
          />
        </div>
      </div>
    </div>
  );
}

interface AttachmentPreviewProps {
  attachment: FileAttachment;
  onRemove: () => void;
}

function AttachmentPreview({ attachment, onRemove }: AttachmentPreviewProps) {
  const [showPreview, setShowPreview] = useState(false);

  return (
    <div className="relative bg-background rounded-md p-1 flex items-center w-full space-x-2 border border-gray-300 dark:border-border">
      <Button
        variant="ghost"
        size="sm"
        className="absolute -top-1.5 -right-1.5 h-4 w-4 rounded-full p-0"
        onClick={onRemove}
      >
        <X className="h-3 w-3 text-gray-900 dark:text-gray-300  bg-gray-300 dark:bg-gray-700 rounded-full" />
      </Button>

      {attachment.preview ? (
        <Dialog open={showPreview} onOpenChange={setShowPreview}>
          <DialogTrigger asChild>
            <img
              src={attachment.preview}
              alt="preview"
              className="h-6 w-6 object-cover rounded cursor-pointer hover:opacity-80 transition-opacity"
            />
          </DialogTrigger>
          <DialogContent className="max-w-3xl p-0 overflow-hidden">
            <img
              src={attachment.preview}
              alt="preview"
              className="w-full h-full object-contain"
            />
          </DialogContent>
        </Dialog>
      ) : (
        <div className="h-6 w-6 bg-muted rounded flex items-center justify-center flex-shrink-0">
          {attachment.type === "pdf" && <FileText className="h-4 w-4" />}
          {attachment.type === "csv" && <Database className="h-4 w-4" />}
          {attachment.type === "spreadsheet" && <Table className="h-4 w-4" />}
          {attachment.type === "text" && <File className="h-4 w-4" />}
          {attachment.type === "unknown" && (
            <FileQuestion className="h-4 w-4" />
          )}
        </div>
      )}
      <div className="flex flex-col min-w-0 text-primary-foreground">
        <p className="text-xs truncate font-medium">{attachment.file.name}</p>
        <p className="text-xs">{(attachment.file.size / 1024).toFixed(1)} KB</p>
      </div>
    </div>
  );
}

interface ChatInputToolBarProps {
  handleProviderSelect: (providerId: string) => void;
  selectedProvider: string;
  providers: Provider[];
  setOpenTools: (val: boolean) => void;
  onAttachClick: () => void;
}

function ChatInputToolBar(props: ChatInputToolBarProps) {
  const {
    handleProviderSelect,
    selectedProvider,
    providers,
    setOpenTools,
    onAttachClick,
  } = props;

  return (
    <div className="pt-2 px-4 space-x-2">
      <Button variant="ghost" size="sm" onClick={onAttachClick}>
        <Paperclip />
      </Button>
      <ModelSelect
        handleProviderSelect={handleProviderSelect}
        selectedProvider={selectedProvider}
        providers={providers}
      />
      <Button
        variant="ghost"
        className="text-xs"
        size="sm"
        onClick={() => setOpenTools(true)}
      >
        <Wrench size={16} /> Tools
      </Button>
    </div>
  );
}
