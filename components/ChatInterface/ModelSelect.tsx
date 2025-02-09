import { Provider } from "src/types";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "../../components/ui/select";
import { Box, Plus } from "lucide-react";
import { Separator } from "../../components/ui/separator";

interface Props {
  handleProviderSelect: (providerId: string) => void;
  selectedProvider: string;
  providers: Provider[];
}
export default function ModelSelect(props: Props) {
  const { handleProviderSelect, selectedProvider, providers } = props;

  return (
    <div className="inline-flex flex-row items-center text-primary-foreground hover:bg-primary/90 rounded-md py-0 px-1">
      <Select onValueChange={handleProviderSelect} value={selectedProvider}>
        <SelectTrigger className="border-0 shadow-none text-xs ring-0 focus:outline-none focus:ring-0 flex flex-row items-center gap-1 h-8">
          <Box size={16} />
          <SelectValue placeholder="Select a model" />
        </SelectTrigger>
        <SelectContent position="popper" side="top" align="start">
          {providers.map((provider) => (
            <SelectItem
              key={provider.id}
              value={provider.id?.toString() || ""}
              className="text-xs"
            >
              {provider.model}
            </SelectItem>
          ))}
          <Separator className="my-1" />
          <SelectItem
            value="new-model"
            className="text-xs transition-colors mt-1"
          >
            <span className="flex items-center gap-1.5">
              <Plus size={12} />
              New Model
            </span>
          </SelectItem>
        </SelectContent>
      </Select>
    </div>
  );
}
