import { getCurrentWindow } from "@tauri-apps/api/window";

export async function handleShortcut() {
  const appWindow = getCurrentWindow();

  try {
    const isVisible = await appWindow.isVisible();
    console.log("Window is currently visible:", isVisible);

    if (isVisible) {
      // If window is visible, hide it
      await appWindow.hide();
      console.log("Window hidden");
    } else {
      // If window is hidden, show it and focus
      await appWindow.show();

      // Focus the window
      await appWindow.setFocus();
      console.log("Window shown and focused");

      // Handle minimized state
      const isMinimized = await appWindow.isMinimized();
      if (isMinimized) {
        await appWindow.unminimize();
        console.log("Window unminimized");
      }
    }
  } catch (error) {
    console.error("Error handling window visibility:", error);
  }
}
