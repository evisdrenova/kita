import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
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
