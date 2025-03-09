import { getCurrentWindow } from "@tauri-apps/api/window";

// Debounce flag to prevent multiple rapid triggers
let isHandlingShortcut = false;
let lastActionTimestamp = 0;
const DEBOUNCE_TIMEOUT = 300; // milliseconds

export async function handleShortcut() {
  const now = Date.now();

  if (isHandlingShortcut || now - lastActionTimestamp < DEBOUNCE_TIMEOUT) {
    console.log("Shortcut trigger ignored - debounced");
    return;
  }

  isHandlingShortcut = true;
  lastActionTimestamp = now;

  const appWindow = getCurrentWindow();

  try {
    const isVisible = await appWindow.isVisible();

    if (isVisible) {
      await appWindow.hide();
    } else {
      await appWindow.show();

      await appWindow.setFocus();

      const isMinimized = await appWindow.isMinimized();
      if (isMinimized) {
        await appWindow.unminimize();
      }
    }
  } catch (error) {
    console.error("Error handling window visibility:", error);
  } finally {
    setTimeout(() => {
      isHandlingShortcut = false;
    }, DEBOUNCE_TIMEOUT);
  }
}
