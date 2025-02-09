import { ServerConfig } from "src/types";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "../ui/table";
import { Button } from "../ui/button";
import { ArrowTopRightIcon } from "@radix-ui/react-icons";
import { Switch } from "../ui/switch";

interface ServerTableProps {
  servers: ServerConfig[];
  handleEdit: (server: ServerConfig) => void;
  handleEnableDisableSwitch: (id: number, val: boolean) => void;
}

export default function ServerTable(props: ServerTableProps) {
  const { servers, handleEdit, handleEnableDisableSwitch } = props;
  return (
    <div className="grid gap-4">
      <Table>
        <TableHeader>
          <TableRow className="text-xs">
            <TableHead className="w-[30%]">Name</TableHead>
            <TableHead className="w-[60%]">Description</TableHead>
            <TableHead className="w-[10%]">Status</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {servers.map((server) => (
            <TableRow key={server.id} className="text-xs">
              <TableCell className="font-medium text-left">
                <Button
                  variant="ghost"
                  size="icon"
                  onClick={() => handleEdit(server)}
                  className="justify-start w-full text-xs hover:bg-transparent hover:no-underline"
                >
                  <div className="flex items-center space-x-2">
                    <span className="hover:underline">{server.name}</span>
                    <ArrowTopRightIcon className="h-4 w-4" />
                  </div>
                </Button>
              </TableCell>
              <TableCell className="text-xs">{server.description}</TableCell>
              <TableCell>
                <Switch
                  checked={server.enabled}
                  onCheckedChange={(checked) =>
                    handleEnableDisableSwitch(server.id, checked)
                  }
                />
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </div>
  );
}
