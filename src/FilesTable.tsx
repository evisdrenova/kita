import { memo, useCallback, useState } from "react";
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
  Copy,
  CheckCircle,
} from "lucide-react";
import { FileMetadata, SemanticMetadata } from "./types/types";
import { cn, FormatFileSize, truncateFilename } from "./lib/utils";
import { FaRegFilePdf } from "react-icons/fa";
import { Badge } from "./components/ui/badge";

interface Props {
  data: FileMetadata[];
  onRowClick?: (file: FileMetadata) => void;
  selectedItemName?: string;
  semanticMatches?: Record<string, SemanticMetadata>;
}

interface Column<T> {
  key: string;
  header: string;
  width: number;
  render?: (item: T) => React.ReactNode;
}

// We'll define the columns inside the component to access the props
const getColumns = (
  semanticMatches?: Record<string, SemanticMetadata>
): Column<FileMetadata>[] => [
  {
    key: "name",
    header: "Name",
    width: 60,
    render: (file) => {
      const matchId = file.id || "";
      const semanticMatch =
        matchId && semanticMatches ? semanticMatches[matchId] : undefined;
      const isSemanticMatch = !!semanticMatch;

      return (
        <div className="flex flex-col min-w-0 max-w-md">
          <div className="flex items-center space-x-2 overflow-hidden text-white">
            {getFileIcon(file.extension)}
            <span className="text-sm truncate text-white">
              {truncateFilename(file.name, 40, true)}
            </span>
            {file.path && (
              <div className="flex items-center text-xs text-gray-400">
                <CopyPathButton path={file.path} />
              </div>
            )}
            {isSemanticMatch && semanticMatch && (
              <SemanticRelevance distance={semanticMatch.distance} />
            )}
          </div>
          {/* {file.path && (
            <div className="flex items-center text-xs text-gray-500 ml-6">
              <span className="truncate mr-1">{truncatePath(file.path)}</span>
              <CopyPathButton path={file.path} />
            </div>
          )} */}
          {isSemanticMatch && semanticMatch?.content && (
            <div className="text-xs text-gray-400 truncate ml-6 line-clamp-1">
              {semanticMatch.content}
            </div>
          )}
        </div>
      );
    },
  },
  {
    key: "size",
    header: "Size",
    width: 20,
    render: (file) => (
      <div className="flex items-center justify-start gap-1 text-xs text-gray-200">
        {FormatFileSize(file.size)}
      </div>
    ),
  },
  {
    key: "type",
    header: "Type",
    width: 20,
    render: (file) => (
      <div className="flex items-center justify-start gap-1 text-xs text-gray-200">
        {file.extension ? file.extension.toUpperCase() : "—"}
      </div>
    ),
  },
];

export default function FilesTable(props: Props) {
  const { data, onRowClick, selectedItemName, semanticMatches = {} } = props;

  // Generate columns with access to semanticMatches
  const columns = getColumns(semanticMatches);
  const [sortKey, setSortKey] = useState<string | null>("name");
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
            setSortDirection("asc");
            return key;
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
  const sortedFiles = [...data].sort((a, b) => {
    if (sortKey === "name") {
      return sortDirection === "asc"
        ? a.name.localeCompare(b.name)
        : b.name.localeCompare(a.name);
    } else if (sortKey === "size") {
      return sortDirection === "asc" ? a.size - b.size : b.size - a.size;
    } else if (sortKey === "type") {
      const typeA = a.extension || "";
      const typeB = b.extension || "";
      return sortDirection === "asc"
        ? typeA.localeCompare(typeB)
        : typeB.localeCompare(typeA);
    }
    return 0;
  });

  return (
    <>
      {sortedFiles.length > 0 && (
        <div className="flex flex-col">
          <div className="bg-inherit">
            <table
              className="w-full border-collapse"
              style={{ tableLayout: "fixed" }}
            >
              <thead>
                <tr>
                  {columns.map((column) => (
                    <th
                      key={column.key}
                      className="text-left p-2 text-xs font-medium text-gray-400"
                      onClick={() => handleSort(column.key)}
                      style={{
                        cursor: "pointer",
                        width: `${column.width}%`,
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
            </table>
          </div>
          <div>
            <table
              className="w-full border-collapse"
              style={{ tableLayout: "fixed" }}
            >
              <tbody>
                {sortedFiles.map((file) => (
                  <TableRow
                    key={`${file.path}-${file.name}`}
                    file={file}
                    columns={columns}
                    onRowClick={onRowClick}
                    isSelected={file.name === selectedItemName}
                    semanticMatch={
                      file.id ? semanticMatches?.[file.id] : undefined
                    }
                  />
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}
    </>
  );
}

const TableRow = memo(
  ({
    file,
    columns,
    onRowClick,
    isSelected,
  }: {
    file: FileMetadata;
    columns: Column<FileMetadata>[];
    onRowClick?: (file: FileMetadata) => void;
    isSelected: boolean;
    semanticMatch?: SemanticMetadata;
  }) => {
    const handleClick = useCallback(() => {
      if (onRowClick) onRowClick(file);
    }, [file, onRowClick]);

    const renderCells = (column: Column<FileMetadata>) => {
      if (column.render) {
        return column.render(file);
      } else {
        return (file as any)[column.key];
      }
    };

    return (
      <tr
        onClick={handleClick}
        className={cn(
          "hover:bg-zinc-200 dark:hover:bg-zinc-800 transition-colors cursor-pointer mb-2",
          isSelected ? "bg-muted" : ""
        )}
        style={{ borderRadius: "8px", overflow: "hidden" }}
      >
        {columns.map((column, index) => (
          <td
            key={column.key}
            className={cn(
              "p-2",
              index === 0 ? "rounded-l" : "",
              index === columns.length - 1 ? "rounded-r" : ""
            )}
            style={{ width: `${column.width}%` }}
          >
            {renderCells(column)}
          </td>
        ))}
      </tr>
    );
  }
);

function SemanticRelevance({ distance }: { distance: number }) {
  // Convert cosine distance (0..2) to a [0..1] similarity
  // Then turn into a [0..100] percentage
  const similarity = 1 - distance / 2;
  const similarityPercentage = Math.round(similarity * 100);

  let matchStrength: string;
  let variantClass: string;

  if (similarityPercentage > 80) {
    matchStrength = "Strong match";
    variantClass =
      "bg-green-900/30 text-green-400 hover:bg-green-900/20 border-green-800";
  } else if (similarityPercentage >= 50) {
    matchStrength = "Good match";
    variantClass =
      "bg-blue-900/30 text-blue-400 hover:bg-blue-900/20 border-blue-800";
  } else {
    matchStrength = "Weak match";
    variantClass =
      "bg-amber-900/30 text-amber-400 hover:bg-amber-900/20 border-amber-800";
  }

  return (
    <Badge
      variant="outline"
      className={cn("text-xs font-normal", variantClass)}
    >
      {matchStrength}
    </Badge>
  );
}

const CopyPathButton = memo(({ path }: { path: string }) => {
  const [copied, setCopied] = useState(false);

  const handleCopy = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      navigator.clipboard
        .writeText(path)
        .then(() => {
          setCopied(true);
          setTimeout(() => setCopied(false), 2000);
        })
        .catch((err) => {
          console.error("Failed to copy path:", err);
        });
    },
    [path]
  );

  return (
    <button
      onClick={handleCopy}
      className="p-1 rounded-sm hover:bg-gray-200 hover:text-white dark:hover:bg-gray-700 transition-colors"
      title="Copy full path"
    >
      {copied ? (
        <CheckCircle className="h-3 w-3 text-green-500" />
      ) : (
        <Copy className="h-3 w-3" />
      )}
    </button>
  );
});

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
