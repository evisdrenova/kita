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
} from "lucide-react";
import { FileMetadata } from "./types/types";
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
}

export default function FilesTable(props: Props) {
  const { data, onRowClick, selectedItemName } = props;

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
  }: {
    file: FileMetadata;
    onRowClick?: (file: FileMetadata) => void;
    isSelected: boolean;
  }) => {
    const handleClick = useCallback(() => {
      if (onRowClick) onRowClick(file);
    }, [file, onRowClick]);

    return (
      <div
        onClick={handleClick}
        className={cn(
          isSelected ? "bg-muted" : "hover:bg-zinc-800/80",
          "transition-colors cursor-pointer rounded "
        )}
      >
        <div className="flex flex-row justify-between w-full p-2 ">
          <div className="flex flex-col gap-1">
            <FileName file={file} />
            <div className="ml-6">
              <FilePath file={file} />
            </div>
          </div>
          <div className="flex flex-col gap-1">
            <FileExtension file={file} />
            <FileSize file={file} />
          </div>
        </div>
      </div>
    );
  }
);

function FileName({ file }: { file: FileMetadata }) {
  return (
    <div className="flex items-center flex-row gap-2 ">
      {getFileIcon(file.extension)}
      <span className="text-sm truncate">
        {truncateFilename(file.name, 40, true)}
      </span>
    </div>
  );
}

function FilePath({ file }: { file: FileMetadata }) {
  return (
    <div className="flex items-center justify-start gap-1 text-[12px] text-gray-500">
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
