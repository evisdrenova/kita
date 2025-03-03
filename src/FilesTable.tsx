import React, { useState, useMemo, memo, useCallback } from "react";
import {
  File,
  FileText,
  FileCode,
  FileSpreadsheet,
  Image,
  Film,
  Music,
  Package,
  Database,
  FileArchive,
} from "lucide-react";
import { FileMetadata, Column } from "./types/types";
import { FormatFileSize, truncateFilename, truncatePath } from "./lib/utils";
import { FaRegFilePdf } from "react-icons/fa";

interface Props {
  data: FileMetadata[];
  onRowClick?: (file: FileMetadata) => void;
}

const columns: Column<FileMetadata>[] = [
  {
    key: "name",
    header: "File Name",
    width: 40,
    render: (file) => (
      <div className="flex items-center flex-row gap-2 ">
        {getFileIcon(file.extension)}
        <span className="text-sm truncate">
          {truncateFilename(file.name, 40, true)}
        </span>
      </div>
    ),
  },
  {
    key: "path",
    header: "Path",
    width: 40,
    render: (file) => (
      <div className="flex items-center justify-start gap-1 text-xs text-gray-500">
        {truncatePath(file.path)}
      </div>
    ),
  },
  {
    key: "size",
    header: "Size",
    width: 15,
    render: (file) => (
      <div className="flex items-center justify-start gap-1 text-xs text-gray-500">
        {FormatFileSize(file.size)}
      </div>
    ),
  },
  {
    key: "extension",
    header: "Type",
    width: 15,
    render: (file) => (
      <div className="flex items-center justify-start gap-1 text-xs text-gray-500">
        {file.extension ? file.extension.toUpperCase() : "—"}
      </div>
    ),
  },
];

export default function FilesTable(props: Props) {
  const { data, onRowClick } = props;

  const [sortKey, setSortKey] = useState<string | null>(null);
  const [sortDirection, setSortDirection] = useState<"asc" | "desc">("asc");

  // Handle column sorting
  const handleSort = useCallback(
    (key: string) => {
      setSortKey((prevKey) => {
        if (prevKey === key) {
          // If already sorting by this key, cycle through directions
          if (sortDirection === "asc") {
            setSortDirection("desc");
            return key;
          } else {
            // If already at desc, go back to no sort
            return null;
          }
        } else {
          // New column, start with ascending sort
          setSortDirection("asc");
          return key;
        }
      });
    },
    [sortDirection]
  );

  // Sort the files
  const sortedFiles = useMemo(() => {
    // If no sort key or in default state, sort alphabetically by name
    if (!sortKey) {
      return [...data].sort((a, b) => {
        return a.name.toLowerCase().localeCompare(b.name.toLowerCase());
      });
    }

    // Sort based on selected column
    return [...data].sort((a, b) => {
      if (sortKey === "size") {
        return sortDirection === "asc" ? a.size - b.size : b.size - a.size;
      }

      if (sortKey === "extension") {
        return sortDirection === "asc"
          ? a.extension.toLowerCase().localeCompare(b.extension.toLowerCase())
          : b.extension.toLowerCase().localeCompare(a.extension.toLowerCase());
      }

      if (sortKey === "updated_at") {
        const dateA = a.updated_at ? new Date(a.updated_at).getTime() : 0;
        const dateB = b.updated_at ? new Date(b.updated_at).getTime() : 0;
        return sortDirection === "asc" ? dateA - dateB : dateB - dateA;
      }

      if (sortKey === "name") {
        return sortDirection === "asc"
          ? a.name.toLowerCase().localeCompare(b.name.toLowerCase())
          : b.name.toLowerCase().localeCompare(a.name.toLowerCase());
      }

      return 0;
    });
  }, [data, sortKey, sortDirection]);

  return (
    <div className="table-container" style={{ overflowX: "auto" }}>
      <table
        className="w-full border-collapse"
        style={{ tableLayout: "fixed" }}
      >
        <colgroup>
          {columns.map((column) => (
            <col key={column.key} style={{ width: `${column.width}%` }} />
          ))}
        </colgroup>
        <thead>
          <tr>
            {columns.map((column) => (
              <th
                key={column.key}
                className="text-left p-2 text-sm font-medium text-gray-500"
                onClick={() => handleSort(column.key)}
                style={{
                  cursor: "pointer",
                }}
              >
                {column.header}
                {sortKey === column.key && (
                  <span className="ml-1">
                    {sortDirection === "asc" ? "↑" : "↓"}
                  </span>
                )}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {sortedFiles.map((file) => (
            <FileRow
              key={`${file.path}-${file.name}`}
              file={file}
              columns={columns}
              onRowClick={onRowClick}
            />
          ))}
        </tbody>
      </table>
    </div>
  );
}

const FileRow = memo(
  ({
    file,
    columns,
    onRowClick,
  }: {
    file: FileMetadata;
    columns: Column<FileMetadata>[];
    onRowClick?: (file: FileMetadata) => void;
  }) => {
    const handleClick = useCallback(() => {
      if (onRowClick) onRowClick(file);
    }, [file, onRowClick]);

    return (
      <tr
        onClick={handleClick}
        className="hover:bg-muted transition-colors cursor-pointer"
      >
        {columns.map((column) => (
          <td key={column.key} className="p-2 truncate">
            {column.render ? column.render(file) : (file as any)[column.key]}
          </td>
        ))}
      </tr>
    );
  }
);

function getFileIcon(filePath: string) {
  const extension =
    filePath.split(".").length > 1
      ? `.${filePath.split(".").pop()?.toLowerCase()}`
      : "";

  let icon;
  switch (extension) {
    case ".app":
    case ".exe":
    case ".dmg":
      icon = <Package className="h-4 w-4" />;
      break;
    case ".pdf":
      icon = <FaRegFilePdf className="h-4 w-4" />;
      break;
    case ".doc":
    case ".docx":
    case ".txt":
    case ".rtf":
      icon = <FileText className="h-4 w-4" />;
      break;
    case ".jpg":
    case ".jpeg":
    case ".png":
    case ".gif":
    case ".svg":
    case ".webp":
      icon = <Image className="h-4 w-4" />;
      break;
    case ".js":
    case ".ts":
    case ".jsx":
    case ".tsx":
    case ".py":
    case ".java":
    case ".cpp":
    case ".html":
    case ".css":
      icon = <FileCode className="h-4 w-4" />;
      break;
    case ".mp4":
    case ".mov":
    case ".avi":
    case ".mkv":
      icon = <Film className="h-4 w-4" />;
      break;
    case ".mp3":
    case ".wav":
    case ".flac":
    case ".m4a":
      icon = <Music className="h-4 w-4" />;
      break;
    case ".json":
    case ".xml":
    case ".yaml":
    case ".yml":
      icon = <Database className="h-4 w-4" />;
      break;
    case ".xlsx":
    case ".xls":
    case ".csv":
      icon = <FileSpreadsheet className="h-4 w-4" />;
      break;
    case ".zip":
    case ".rar":
    case ".7z":
    case ".tar":
    case ".gz":
      icon = <FileArchive className="h-4 w-4" />;
      break;
    default:
      icon = <File className="h-4 w-4" />;
  }

  return icon;
}
