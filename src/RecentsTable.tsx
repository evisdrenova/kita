export default function Recents() {
  return <div></div>;
}

// function Recents(props: RecentsProps) {
//   const { recents, handleResultSelect } = props;

//   return (
//     <div>
//       <h2 className="text-xs font-semibold text-muted-foreground mb-2">
//         Recent Files
//       </h2>
//       <div className="flex flex-col">
//         {recents.map((file, index) => (
//           <div
//             key={index}
//             className="flex items-center cursor-pointer hover:bg-muted p-2 rounded-md group"
//             onClick={() => handleResultSelect(file, SearchSectionType.Files)}
//           >
//             <div className="flex items-center gap-2 min-w-0 flex-1">
//               <div className="flex flex-col min-w-0 flex-1">
//                 <div className="flex flex-row items-center gap-1">
//                   {getFileIcon(file.path)}
//                   <span className="text-sm">{file.name}</span>
//                 </div>
//                 <div className="flex items-center gap-2 min-w-0 h-0 group-hover:h-auto overflow-hidden transition-all duration-200">
//                   <span className="text-xs text-muted-foreground whitespace-nowrap overflow-hidden text-ellipsis pl-5 flex-1">
//                     {truncatePath(file.path)}
//                   </span>
//                   <span className="text-xs text-muted-foreground whitespace-nowrap">
//                     {getCategoryFromExtension(file.extension)}
//                   </span>
//                 </div>
//               </div>
//             </div>
//           </div>
//         ))}
//       </div>
//     </div>
//   );
// }
