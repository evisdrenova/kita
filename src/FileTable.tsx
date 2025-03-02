import {
  Cpu,
  Database,
  File,
  FileArchive,
  FileCode,
  FileSpreadsheet,
  FileText,
  Film,
  Image,
  Loader2,
  MemoryStick,
  Music,
  Package,
  RefreshCw,
  Search,
  X,
} from "lucide-react";

export default function FileTable() {
  return <div></div>;
}

// interface FileRowProps {
//   file: Extract<SearchItem, { type: SearchSectionType.Files }>;
//   handleCopy: (path: string, id: number) => Promise<void>;
//   copiedId: number | null;
// }

// function FileRow(props: FileRowProps) {
//   const { file, handleCopy, copiedId } = props;

//   return (
//     <div className="flex flex-col w-full flex-1 gap-3">
//       <div className="flex flex-row justify-between w-full items-center gap-1">
//         <div className="flex flex-row w-full items-center gap-1">
//           {getFileIcon(file.path)}
//           <span className="text-sm text-primary-foreground">{file.name}</span>
//           <button
//             onClick={(e) => {
//               e.stopPropagation();
//               // handleCopy(file.path, file.id);
//             }}
//             className={`opacity-0 group-hover:opacity-100 ml-2 p-1 hover:bg-background rounded transition-opacity duration-200 ${
//               copiedId === file.id ? "text-green-500" : "text-muted-foreground"
//             }`}
//           >
//             {copiedId === file.id ? (
//               <Check className="h-3 w-3" />
//             ) : (
//               <Copy className="h-3 w-3" />
//             )}
//           </button>
//         </div>
//         <span className="text-xs text-muted-foreground whitespace-nowrap">
//           {getCategoryFromExtension(file.extension)}
//         </span>
//       </div>
//       <div className="flex justify-between items-center gap-2 w-full h-0">
//         <span className="text-xs text-muted-foreground whitespace-nowrap overflow-hidden text-ellipsis pl-4 flex-1">
//           {truncatePath(file.path)}
//         </span>
//         <span className="text-xs text-muted-foreground whitespace-nowrap">
//           {FormatFileSize(file.size)}
//         </span>
//       </div>
//     </div>
//   );
// }

// interface SemanticRowProps {
//   file: Extract<SearchItem, { type: SearchSectionType.Semantic }>;
//   handleCopy: (path: string, id: number) => Promise<void>;
//   copiedId: number | null;
// }

// function SemanticRow(props: SemanticRowProps) {
//   const { file, handleCopy, copiedId } = props;

//   return (
//     <div className="flex items-center gap-2 min-w-0 flex-1">
//       <div className="flex flex-col min-w-0 flex-1">
//         <div className="flex flex-row items-center gap-1">
//           {getFileIcon(file.path)}
//           <span className="text-sm text-primary-foreground">{file.name}</span>
//           <span className="pl-2">{Math.floor(file.distance * 100)}%</span>
//           {
//             <button
//               onClick={(e) => {
//                 e.stopPropagation();
//                 // handleCopy(file.path, file.id);
//               }}
//               className={`opacity-0 group-hover:opacity-100 ml-2 p-1 hover:bg-background rounded transition-opacity duration-200 ${
//                 copiedId === file.id
//                   ? "text-green-500"
//                   : "text-muted-foreground"
//               }`}
//             >
//               {copiedId === file.id ? (
//                 <Check className="h-3 w-3" />
//               ) : (
//                 <Copy className="h-3 w-3" />
//               )}
//             </button>
//           }
//         </div>
//         <div className="flex items-center gap-2 min-w-0 h-0 group-hover:h-auto overflow-hidden transition-all duration-200">
//           {
//             <span className="text-xs text-muted-foreground whitespace-nowrap overflow-hidden text-ellipsis pl-5 flex-1">
//               {truncatePath(file.path)}
//             </span>
//           }
//           {
//             <span className="text-xs text-muted-foreground whitespace-nowrap">
//               {getCategoryFromExtension(file.extension)}
//             </span>
//           }
//         </div>
//       </div>
//     </div>
//   );
// }

// interface RecentsProps {
//   recents: FileMetadata[];
//   handleResultSelect: (
//     item: SearchItem,
//     type: SearchSectionType
//   ) => Promise<void>;
// }

// function getFileIcon(filePath: string) {
//   const extension =
//     filePath.split(".").length > 1
//       ? `.${filePath.split(".").pop()?.toLowerCase()}`
//       : "";

//   let icon;
//   switch (extension) {
//     case ".app":
//     case ".exe":
//     case ".dmg":
//       icon = <Package className="h-3 w-3" />;
//       break;
//     case ".pdf":
//       icon = <FaRegFilePdf className="h-3 w-3" />;
//       break;
//     case ".doc":
//     case ".docx":
//     case ".txt":
//     case ".rtf":
//       icon = <FileText className="h-3 w-3" />;
//       break;
//     case ".jpg":
//     case ".jpeg":
//     case ".png":
//     case ".gif":
//     case ".svg":
//     case ".webp":
//       icon = <Image className="h-3 w-3" />;
//       break;
//     case ".js":
//     case ".ts":
//     case ".jsx":
//     case ".tsx":
//     case ".py":
//     case ".java":
//     case ".cpp":
//     case ".html":
//     case ".css":
//       icon = <FileCode className="h-3 w-3" />;
//       break;
//     case ".mp4":
//     case ".mov":
//     case ".avi":
//     case ".mkv":
//       icon = <Film className="h-3 w-3" />;
//       break;
//     case ".mp3":
//     case ".wav":
//     case ".flac":
//     case ".m4a":
//       icon = <Music className="h-3 w-3" />;
//       break;
//     case ".json":
//     case ".xml":
//     case ".yaml":
//     case ".yml":
//       icon = <Database className="h-3 w-3" />;
//       break;
//     case ".xlsx":
//     case ".xls":
//     case ".csv":
//       icon = <FileSpreadsheet className="h-3 w-3" />;
//       break;
//     case ".zip":
//     case ".rar":
//     case ".7z":
//     case ".tar":
//     case ".gz":
//       icon = <FileArchive className="h-3 w-3" />;
//       break;
//     default:
//       icon = <File className="h-3 w-3" />;
//   }

//   return icon;
// }
