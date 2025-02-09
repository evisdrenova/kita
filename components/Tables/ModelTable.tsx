import { Provider } from "src/types";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "../../components/ui/table";
import { Button } from "../../components/ui/button";
import { ArrowTopRightIcon } from "@radix-ui/react-icons";

interface ModelTableProps {
  models: Provider[];
  handleEdit: (Model: Provider) => void;
}

export default function ModelTable(props: ModelTableProps) {
  const { models, handleEdit } = props;
  return (
    <div className="grid gap-4">
      <Table>
        <TableHeader>
          <TableRow className="text-xs">
            <TableHead className="w-[33%]">Name</TableHead>
            <TableHead className="w-[33%]">Type</TableHead>
            <TableHead className="w-[33%]">Model</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {models.map((model) => (
            <TableRow key={model.id} className="text-xs">
              <TableCell className="font-medium text-left">
                <Button
                  variant="ghost"
                  size="icon"
                  onClick={() => handleEdit(model)}
                  className="justify-start w-full text-xs hover:bg-transparent hover:no-underline"
                >
                  <div className="flex items-center space-x-2">
                    <span className="hover:underline">{model.name}</span>
                    <ArrowTopRightIcon className="h-4 w-4" />
                  </div>
                </Button>
              </TableCell>
              <TableCell className="text-xs">{model.type}</TableCell>
              <TableCell className="text-xs">{model.model}</TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </div>
  );
}
