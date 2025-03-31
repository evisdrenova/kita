import { listen } from "@tauri-apps/api/event";
import { useEffect } from "react";

export default function ModelManager() {
  useEffect(() => {
    // Listen for model selection required event
    const unlistenSelection = listen("model-selection-required", (event) => {
      // Show a dialog or notification prompting user to select a model
      console.log("Model selection required:", event.payload);
      // Example: openModelSelectionDialog(event.payload);
    });

    // Listen for model download required event
    const unlistenDownload = listen("model-download-required", (event) => {
      // Show a dialog or notification prompting user to download the model
      console.log("Model download required:", event.payload);
      // Example: showDownloadPrompt(event.payload);
    });

    return () => {
      unlistenSelection.then((fn) => fn());
      unlistenDownload.then((fn) => fn());
    };
  }, []);

  return <div>select a model please</div>;
}
