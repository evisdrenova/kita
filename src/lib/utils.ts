import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";
import { SearchCategory } from "@/src/types/types";

export function cn(...inputs: ClassValue[]): string {
  return twMerge(clsx(inputs));
}

export function toTitleCase(s: string): string {
  if (s) {
    const first = s.substring(0, 1);
    const rest = s.substring(1);
    return `${first.toUpperCase()}${rest}`;
  }
  return "";
}

export function debounce<T extends (...args: any[]) => any>(
  fn: T,
  delay: number
): (...args: Parameters<T>) => void {
  let timer: NodeJS.Timeout;
  return (...args: Parameters<T>) => {
    if (timer) clearTimeout(timer);
    timer = setTimeout(() => fn(...args), delay);
  };
}

export function getCategoryFromExtension(extension: string): SearchCategory {
  if (!extension || typeof extension !== "string") {
    return "Other";
  }

  switch (extension.toLowerCase()) {
    case ".app":
    case ".exe":
    case ".dmg":
      return "Applications";

    case ".pdf":
      return "PDF Documents";

    case ".doc":
    case ".docx":
    case ".txt":
    case ".rtf":
      return "Documents";

    case ".jpg":
    case ".jpeg":
    case ".png":
    case ".gif":
    case ".svg":
    case ".webp":
      return "Images";

    case ".js":
    case ".ts":
    case ".jsx":
    case ".tsx":
    case ".py":
    case ".java":
    case ".cpp":
    case ".html":
    case ".css":
    case ".json":
    case ".xml":
    case ".yaml":
    case ".yml":
      return "Documents";

    case ".mp4":
    case ".mov":
    case ".avi":
    case ".mkv":
      return "Other";

    case ".mp3":
    case ".wav":
    case ".flac":
    case ".m4a":
      return "Other";

    case ".xlsx":
    case ".xls":
    case ".csv":
      return "Spreadsheets";

    case ".zip":
    case ".rar":
    case ".7z":
    case ".tar":
    case ".gz":
      return "Other";

    default:
      return "Other";
  }
}

export function truncatePath(path: string, maxLength: number = 50) {
  const parts = path.split("/");
  const fileName = parts.pop() || "";
  const directory = parts.join("/");

  if (path.length <= maxLength) return path;

  // Calculate how many characters we can show from start and end of the directory
  const dotsLength = 3;
  const maxDirLength = maxLength - fileName.length - dotsLength;
  const startLength = Math.floor(maxDirLength / 2);
  const endLength = Math.floor(maxDirLength / 2);

  const startPath = directory.slice(0, startLength);
  const endPath = directory.slice(-endLength);

  return `${startPath}...${endPath}/${fileName}`;
}

// Helper function to check if a string is valid JSON
export function isValidJSON(str: string) {
  try {
    JSON.parse(str);
    return true;
  } catch (e) {
    return false;
  }
}

export function FormatFileSize(bytes: number | undefined): string {
  if (bytes === undefined || bytes === 0) return "0 B";

  const units = ["B", "KB", "MB", "GB", "TB", "PB"];
  const i = Math.floor(Math.log(bytes) / Math.log(1024));

  // Return bytes as is if less than 1 KB
  if (i === 0) return `${bytes} ${units[i]}`;

  // Otherwise format with 2 decimal places
  return `${(bytes / Math.pow(1024, i)).toFixed(2)} ${units[i]}`;
}


