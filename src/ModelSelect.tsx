import { useState, useEffect } from "react";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "./components/ui/select";
import { Skeleton } from "./components/ui/skeleton";
import { Button } from "./components/ui/button";
import { Progress } from "./components/ui/progress";
import { Download, Check, AlertCircle } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

interface Model {
  id: string;
  name: string;
  size: number; //in MB
  quantization: string;
  is_downloaded: boolean;
}

interface DownloadStatus {
  isDownloading: boolean;
  progress: number;
  error: string | null;
  model_id: string | null;
}

interface DownloadProgress {
  progress: number;
  model_id: string;
}

export default function ModelSelect() {
  const [models, setModels] = useState<Model[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [selectedModel, setSelectedModel] = useState<string | null>(null);
  const [downloadStatus, setDownloadStatus] = useState<DownloadStatus>({
    isDownloading: false,
    progress: 0,
    error: null,
    model_id: null,
  });

  // Load available models from backend
  useEffect(() => {
    const fetchModels = async () => {
      try {
        setIsLoading(true);
        const availableModels = await invoke<Model[]>("get_available_models");
        setModels(availableModels);
      } catch (error) {
        console.error("Failed to fetch models:", error);
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

  const handleDownloadModel = async (modelId: string) => {
    try {
      // Reset any previous error
      setDownloadStatus((prev) => ({
        ...prev,
        error: null,
      }));

      // Start download via backend
      await invoke("start_model_download", { modelId });

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

  return (
    <div className="space-y-4">
      <div className="flex items-center gap-4">
        <Select
          value={selectedModel || undefined}
          onValueChange={setSelectedModel}
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
