import { ReactElement } from "react";
import { cn } from "./utils";
import { RefreshCcw } from "lucide-react";

interface Props {
  className?: string;
}

export default function Spinner(props: Props): ReactElement {
  const { className } = props;
  return <RefreshCcw className={cn("animate-spin", className)} />;
}
