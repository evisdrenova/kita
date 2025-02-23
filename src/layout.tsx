import { ReactNode } from "react";
import { Toaster } from "sonner";

interface Props {
  children: ReactNode;
}

export default function Layout(props: Props) {
  const { children } = props;

  return (
    <div className="flex flex-col h-screen">
      <main className="flex-1">{children}</main>
      <Toaster richColors closeButton />
    </div>
  );
}
