import { Contact } from "@/src/types/types";
import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useState } from "react";

interface ContactError {
  message: string;
  code?: string;
  details?: unknown;
}

export function useGetContacts() {
  const [contacts, setContacts] = useState<Contact[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<ContactError | null>(null);

  const fetchContacts = useCallback(async () => {
    setIsLoading(true);
    setError(null);
    try {
      const data = await invoke<Contact[]>("get_contacts");
      setContacts(data);
    } catch (err) {
      if (err instanceof Error) {
        setError({
          message: err.message,
          details: err,
        });
      } else {
        setError({
          message: String(err),
        });
      }
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchContacts();
  }, [fetchContacts]);

  return { contacts, isLoading, error, refreshContacts: fetchContacts };
}
