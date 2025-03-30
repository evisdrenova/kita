import { useState, useEffect } from "react";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "../components/ui/select";
import { Skeleton } from "../components/ui/skeleton";
import { Button } from "../components/ui/button";
import { Progress } from "../components/ui/progress";
import { Input } from "../components/ui/input";
import { Download, Check, AlertCircle, FolderOpen } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Badge } from "../components/ui/badge";

// Interface for model data
interface Model {
  id: string;
  name: string;
  size: number; // Size in MB
  quantization: string;
  is_downloaded: boolean;
}

interface DownloadProgress {
  progress: number;
  model_id: string;
}

export default function Models() {
  const [models, setModels] = useState<Model[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [selectedModel, setSelectedModel] = useState<string | null>(null);
  const [customPath, setCustomPath] = useState<string>("");
  const [useCustomPath, setUseCustomPath] = useState<boolean>(false);
  const [downloadStatus, setDownloadStatus] = useState<{
    isDownloading: boolean;
    progress: number;
    error: string | null;
    model_id: string | null;
  }>({
    isDownloading: false,
    progress: 0,
    error: null,
    model_id: null,
  });

  useEffect(() => {
    const fetchModels = async () => {
      try {
        setIsLoading(true);

        // Fetch available models
        const availableModels = await invoke<Model[]>("get_available_models");

        // Get saved model preference
        const savedModelId = await invoke<string | null>("get_selected_model");

        setModels(availableModels);

        if (savedModelId) {
          setSelectedModel(savedModelId);
        } else if (availableModels.length > 0) {
          // Auto-select first downloaded model if available
          const downloadedModel = availableModels.find((m) => m.is_downloaded);
          if (downloadedModel) {
            setSelectedModel(downloadedModel.id);
            await invoke("set_selected_model", { modelId: downloadedModel.id });
          }
        }
      } catch (error) {
        console.error("Failed to load models:", error);
      } finally {
        setIsLoading(false);
      }
    };

    fetchModels();
  }, []);

  // Listen for download progress events from Rust backend
  useEffect(() => {
    const unlisten1 = listen<DownloadProgress>(
      "model-download-progress",
      (event) => {
        const { progress, model_id } = event.payload;
        setDownloadStatus((prev) => ({
          ...prev,
          isDownloading: true,
          progress,
          model_id,
        }));
      }
    );

    const unlisten2 = listen<string>("model-download-complete", (event) => {
      const model_id = event.payload;
      setDownloadStatus((prev) => ({
        ...prev,
        isDownloading: false,
        progress: 100,
        model_id,
      }));

      // Update the model's downloaded status
      setModels((models) =>
        models.map((model) =>
          model.id === model_id ? { ...model, is_downloaded: true } : model
        )
      );
    });

    const unlisten3 = listen<{ model_id: string; error: string }>(
      "model-download-error",
      (event) => {
        const { model_id, error } = event.payload;
        setDownloadStatus({
          isDownloading: false,
          progress: 0,
          error,
          model_id,
        });
      }
    );

    // Cleanup listeners on component unmount
    return () => {
      unlisten1.then((unsubscribe) => unsubscribe());
      unlisten2.then((unsubscribe) => unsubscribe());
      unlisten3.then((unsubscribe) => unsubscribe());
    };
  }, []);

  const selectModelPath = async () => {
    try {
      // Open a directory selection dialog
      const selected = await open({
        directory: true,
        multiple: false,
        title: "Select Model Storage Location",
      });

      if (selected && typeof selected === "string") {
        setCustomPath(selected);
        setUseCustomPath(true);
      }
    } catch (error) {
      console.error("Failed to select directory:", error);
    }
  };

  const handleDownloadModel = async (modelId: string) => {
    try {
      // Reset any previous error
      setDownloadStatus((prev) => ({
        ...prev,
        error: null,
      }));

      // Start download via backend, passing custom path if specified
      await invoke("start_model_download", {
        modelId,
        customPath: useCustomPath ? customPath : null,
      });

      // The progress, completion, and errors will be handled by event listeners
    } catch (error) {
      console.error("Failed to start download:", error);
      setDownloadStatus({
        isDownloading: false,
        progress: 0,
        error: String(error),
        model_id: modelId,
      });
    }
  };

  if (isLoading) {
    return <Skeleton className="h-10 w-[250px]" />;
  }

  async function handleSetModel(modelId: string) {
    try {
      setSelectedModel(modelId);
      await invoke("set_selected_model", { modelId });
      console.log("Model selection saved:", modelId);
    } catch (error) {
      console.error("Failed to save model selection:", error);
    }
  }

  return (
    <div className="space-y-4">
      <div className="flex items-center gap-4">
        <Select
          value={selectedModel || undefined}
          onValueChange={handleSetModel}
        >
          <SelectTrigger className="w-[250px]">
            <SelectValue placeholder="Select a model" />
          </SelectTrigger>
          <SelectContent>
            {models.map((model) => (
              <SelectItem value={model.id} key={model.id}>
                <div className="flex justify-between items-center w-full">
                  <span>{model.name}</span>
                  <span className="text-xs text-gray-500">
                    {(model.size / 1024).toFixed(1)}GB
                  </span>
                  {model.is_downloaded && <Badge>Downloaded</Badge>}
                </div>
              </SelectItem>
            ))}
          </SelectContent>
        </Select>

        {selectedModel && (
          <Button
            onClick={() => handleDownloadModel(selectedModel)}
            disabled={
              downloadStatus.isDownloading ||
              models.find((m) => m.id === selectedModel)?.is_downloaded
            }
            className="ml-2"
          >
            {models.find((m) => m.id === selectedModel)?.is_downloaded ? (
              <>
                <Check className="mr-2 h-4 w-4" /> Downloaded
              </>
            ) : downloadStatus.isDownloading &&
              downloadStatus.model_id === selectedModel ? (
              <>
                <Download className="mr-2 h-4 w-4 animate-pulse" />{" "}
                Downloading...
              </>
            ) : (
              <>
                <Download className="mr-2 h-4 w-4" /> Download
              </>
            )}
          </Button>
        )}
      </div>

      <div className="pt-2">
        <div className="text-sm font-medium mb-2 block">
          Model Storage Location
          {!useCustomPath && (
            <span className="text-gray-500 ml-2 text-xs">
              (Default: App Data Directory)
            </span>
          )}
        </div>
        <div className="flex gap-2 items-center">
          <Input
            type="text"
            placeholder="Use default app data directory"
            value={customPath}
            onChange={(e) => {
              setCustomPath(e.target.value);
              setUseCustomPath(e.target.value !== "");
            }}
            className="flex-1"
            disabled={downloadStatus.isDownloading}
          />
          <Button
            variant="outline"
            onClick={selectModelPath}
            disabled={downloadStatus.isDownloading}
          >
            <FolderOpen className="h-4 w-4" />
          </Button>
        </div>
      </div>

      {downloadStatus.isDownloading && (
        <div className="space-y-2">
          <div className="flex justify-between text-sm">
            <span>
              Downloading{" "}
              {models.find((m) => m.id === downloadStatus.model_id)?.name}
            </span>
            <span>{downloadStatus.progress.toFixed(0)}%</span>
          </div>
          <Progress value={downloadStatus.progress} className="h-2" />
        </div>
      )}

      {downloadStatus.error && (
        <div className="flex items-center text-red-500 text-sm mt-2">
          <AlertCircle className="h-4 w-4 mr-1" />
          Download failed: {downloadStatus.error}
        </div>
      )}
    </div>
  );
}
