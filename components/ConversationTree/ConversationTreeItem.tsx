import { ChevronRight } from "lucide-react";
import { cn } from "../../src/lib/utils";
import { Button } from "../../components/ui/button";
import { Node } from "./ConversationTree";
import { useRef, useState, useEffect } from "react";

interface Props {
  node: Node;
  isOpen: boolean;
  onToggleOpen: () => void;
  toggleNodeOpen: (id: number) => void;
  openNodes: Record<number, boolean>;
  onSelectConversation: (conversationId: number) => void;
  activeConversationId: number;
}

export default function ConversationTreeItem(props: Props) {
  const {
    node,
    onSelectConversation,
    isOpen,
    onToggleOpen,
    openNodes,
    toggleNodeOpen,
    activeConversationId,
  } = props;

  const hasChildren = node.nodes && node.nodes.length > 0;
  const isActive = node.id == activeConversationId;
  const isChild = node.parentId;

  const childHeightRef = useRef<HTMLDivElement>(null);
  const [childHeight, setChildHeight] = useState<number>(0);

  useEffect(() => {
    const updateHeight = () => {
      if (childHeightRef.current && isOpen) {
        if (node.nodes.length === 1) {
          setChildHeight(0);
        } else {
          // Get the distance between first and last child
          const firstChild = childHeightRef.current.firstChild as HTMLElement;
          const lastChild = childHeightRef.current.lastChild as HTMLElement;
          if (firstChild && lastChild) {
            const distance =
              lastChild.getBoundingClientRect().top -
              firstChild.getBoundingClientRect().top;
            setChildHeight(distance + 14); // Add half the height of the final node
          }
        }
      } else {
        setChildHeight(0);
      }
    };

    updateHeight();

    const resizeObserver = new ResizeObserver(updateHeight);
    if (childHeightRef.current) {
      resizeObserver.observe(childHeightRef.current);
    }

    return () => {
      resizeObserver.disconnect();
    };
  }, [isOpen, node.nodes, openNodes]);

  return (
    <div className="flex flex-col">
      <div className="flex items-center">
        {hasChildren && (
          <button
            onClick={onToggleOpen}
            className="flex items-center justify-center"
          >
            <ChevronRight
              className={cn(
                "size-4 text-gray-500 transition-transform duration-200",
                isOpen && "rotate-90"
              )}
            />
          </button>
        )}
        <div className={cn(hasChildren || isChild ? "pl-0" : "pl-4", "w-full")}>
          <Button
            variant="ghost"
            className={cn(
              "text-xs gap-0 px-1 w-full flex justify-start py-0 h-8",
              isActive && "bg-primary"
            )}
            size="sm"
            onClick={() => onSelectConversation(node.id)}
          >
            {node.name}
          </Button>
        </div>
      </div>
      {isOpen && (
        <div className="flex flex-row w-full min-h-full pl-5 pt-1">
          <div className="relative flex flex-col">
            {node.nodes.length > 1 && (
              <div
                className="absolute left-0 w-[2px] bg-primary/90"
                style={{ height: `${childHeight}px` }}
              />
            )}
            <div className="flex flex-col gap-1" ref={childHeightRef}>
              {node.nodes?.map((childNode) => (
                <div className="flex flex-row gap-0" key={childNode.id}>
                  <div className="flex flex-row gap-0 h-8">
                    {childNode.nodes.length == 0 && (
                      <div className="bg-primary/90 h-1/2 w-[2px]" />
                    )}
                    <div className="bg-primary/90 w-4 h-[2px] mt-[14px]" />
                  </div>
                  <ConversationTreeItem
                    node={childNode}
                    isOpen={!!openNodes[childNode.id]}
                    openNodes={openNodes}
                    toggleNodeOpen={toggleNodeOpen}
                    onToggleOpen={() => toggleNodeOpen(childNode.id)}
                    onSelectConversation={onSelectConversation}
                    activeConversationId={activeConversationId}
                  />
                </div>
              ))}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
