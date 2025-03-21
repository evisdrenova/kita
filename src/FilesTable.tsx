import { memo, useCallback } from "react";
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
  Zap,
} from "lucide-react";
import { FileMetadata, SemanticMetadata } from "./types/types";
import {
  cn,
  FormatFileSize,
  truncateFilename,
  truncatePath,
} from "./lib/utils";
import { FaRegFilePdf } from "react-icons/fa";

interface Props {
  data: FileMetadata[];
  onRowClick?: (file: FileMetadata) => void;
  selectedItemName?: string;
  semanticMatches?: Record<string, SemanticMetadata>;
}

export default function FilesTable(props: Props) {
  const { data, onRowClick, selectedItemName, semanticMatches } = props;

  console.log("data in files table", data);
  return (
    <>
      {data.length > 0 && (
        <div className="flex flex-col gap-1">
          {data.map((file) => (
            <FileRow
              key={`${file.path}-${file.name}`}
              file={file}
              onRowClick={onRowClick}
              isSelected={file.name === selectedItemName}
              semanticMatch={file.id ? semanticMatches?.[file.id] : undefined}
            />
          ))}
        </div>
      )}
    </>
  );
}

const FileRow = memo(
  ({
    file,
    onRowClick,
    isSelected,
    semanticMatch,
  }: {
    file: FileMetadata;
    onRowClick?: (file: FileMetadata) => void;
    isSelected: boolean;
    semanticMatch?: SemanticMetadata;
  }) => {
    const handleClick = useCallback(() => {
      if (onRowClick) onRowClick(file);
    }, [file, onRowClick]);

    const isSemanticMatch = !!semanticMatch;

    return (
      <div
        onClick={handleClick}
        className={cn(
          isSelected ? "bg-muted" : "hover:bg-zinc-800",
          "transition-colors cursor-pointer rounded "
        )}
      >
        <div className="flex flex-row justify-between w-full p-2 ">
          <div className="flex flex-col gap-1">
            <FileName file={file} isSemanticMatch={isSemanticMatch} />
            <div className="ml-6">
              <FilePath file={file} />
              {semanticMatch?.content && (
                <div className="text-xs text-gray-400 mt-1 ml-0.5 line-clamp-1">
                  {semanticMatch.content}
                </div>
              )}
            </div>
          </div>
          <div className="flex flex-col gap-1">
            <div className="flex items-center gap-2">
              <FileExtension file={file} />
              {isSemanticMatch && (
                <SemanticRelevance distance={semanticMatch.distance} />
              )}
            </div>
            <FileSize file={file} />
          </div>
        </div>
      </div>
    );
  }
);

function FileName({
  file,
  isSemanticMatch,
}: {
  file: FileMetadata;
  isSemanticMatch: boolean;
}) {
  return (
    <div className="flex items-center flex-row gap-2 ">
      {getFileIcon(file.extension)}
      <span className="text-sm truncate">
        {truncateFilename(file.name, 40, true)}
      </span>
      {isSemanticMatch && <Zap className="h-3 w-3 text-amber-400" />}
    </div>
  );
}

function FilePath({ file }: { file: FileMetadata }) {
  return (
    <div className="flex items-center justify-start gap-1 text-gray-500">
      {truncatePath(file.path)}
    </div>
  );
}

function FileExtension({ file }: { file: FileMetadata }) {
  return (
    <div className="flex items-center justify-start gap-1 text-xs text-gray-500">
      {file.extension ? file.extension.toUpperCase() : "—"}
    </div>
  );
}

function FileSize({ file }: { file: FileMetadata }) {
  return (
    <div className="flex items-center justify-start gap-1 text-xs text-gray-500">
      {FormatFileSize(file.size)}
    </div>
  );
}

function SemanticRelevance({ distance }: { distance: number }) {
  // Convert distance to similarity percentage (assuming distance is between 0-1)
  // Lower distance means higher similarity
  const similarityPercentage = Math.round((1 - distance) * 100);

  return (
    <div className="flex items-center text-xs px-1.5 py-0.5 rounded-full bg-amber-950/30 text-amber-400">
      {similarityPercentage}% match
    </div>
  );
}

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
