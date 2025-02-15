import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";
import path from "path";
import { SearchCategory } from "src/types";

/**
 * Combines multiple class names or class value objects and merges them with Tailwind CSS classes.
 * Uses clsx for conditional classes and tailwind-merge to handle Tailwind class conflicts.
 *
 * @param inputs - Array of class values (strings, objects, or arrays)
 * @returns Merged and optimized class string
 *
 * @example
 * ```typescript
 * // Basic usage
 * cn('px-2 py-1', 'bg-blue-500');
 * // => 'px-2 py-1 bg-blue-500'
 *
 * // With conditions
 * cn('px-2', { 'bg-blue-500': true, 'bg-red-500': false });
 * // => 'px-2 bg-blue-500'
 *
 * // Merging conflicting Tailwind classes
 * cn('px-2 py-1 bg-red-500', 'bg-blue-500');
 * // => 'px-2 py-1 bg-blue-500'
 * ```
 */
export function cn(...inputs: ClassValue[]): string {
  return twMerge(clsx(inputs));
}

/**
 * Converts a string to Title Case by capitalizing its first character.
 * The rest of the string remains unchanged.
 *
 * @param s - The input string to convert
 * @returns The input string with its first character capitalized
 *
 * @example
 * ```typescript
 * toTitleCase('hello');   // => 'Hello'
 * toTitleCase('world');   // => 'World'
 * toTitleCase('');        // => ''
 * toTitleCase('a');       // => 'A'
 * ```
 */
export function toTitleCase(s: string): string {
  if (s) {
    const first = s.substring(0, 1);
    const rest = s.substring(1);
    return `${first.toUpperCase()}${rest}`;
  }
  return "";
}

/**
 * Creates a debounced version of a function that delays its execution until after
 * a specified delay has elapsed since the last time it was invoked.
 *
 * @param fn - The function to debounce
 * @param delay - The delay in milliseconds
 * @returns A debounced version of the input function
 *
 * @example
 * ```typescript
 * // Basic usage
 * const debouncedSearch = debounce((query: string) => {
 *   performSearch(query);
 * }, 300);
 *
 * // Using with event listeners
 * input.addEventListener('input', (e) => {
 *   debouncedSearch(e.target.value);
 * });
 *
 * // Using with React
 * const handleChange = debounce((value: string) => {
 *   setSearchQuery(value);
 * }, 500);
 * ```
 *
 * @remarks
 * - If the debounced function is called multiple times within the delay period,
 *   only the last call will be executed
 * - The timer is reset each time the function is called
 * - Useful for performance optimization with frequent events like scroll or input
 */
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
  switch (extension.toLowerCase()) {
    case ".app":
      return "Applications";
    case ".pdf":
      return "PDF Documents";
    case ".doc":
    case ".docx":
    case ".txt":
      return "Documents";
    case ".jpg":
    case ".jpeg":
    case ".png":
    case ".gif":
      return "Images";
    default:
      return "Other";
  }
}
