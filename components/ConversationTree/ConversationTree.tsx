import { Conversation } from "../../src/types";
import { Button } from "../ui/button";
import ConversationTreeItem from "./ConversationTreeItem";
import { useState, useMemo, useEffect } from "react";
interface Props {
  conversations: Conversation[];
  onNewConversation: (parentId?: string) => void;
  onSelectConversation: (conversationId: number) => void;
  onDeleteConversation: (convoId: number) => void;
  activeConversationId: number;
}

export interface Node {
  id: number;
  parentId?: number;
  name: string;
  nodes?: Node[];
}

export default function ConversationTree(props: Props) {
  const {
    conversations,
    onNewConversation,
    onSelectConversation,
    activeConversationId,
  } = props;
  // tracks which nodes are open in the convo tree
  const [openNodes, setOpenNodes] = useState<Record<number, boolean>>({});

  const nodes = useMemo(
    () => convertConversationsToNodes(conversations),
    [conversations]
  );

  useEffect(() => {
    if (!activeConversationId) return;

    const expandParents = (nodes: Node[]): void => {
      for (const node of nodes) {
        if (node.id === activeConversationId) {
          return;
        }
        if (node.nodes) {
          const hasActiveChild = node.nodes.some(
            (child) =>
              child.id === activeConversationId ||
              hasActiveDescendant(child, activeConversationId)
          );
          if (hasActiveChild) {
            setOpenNodes((prev) => ({ ...prev, [node.id]: true }));
          }
          expandParents(node.nodes);
        }
      }
    };

    expandParents(nodes);
  }, [activeConversationId, nodes]);

  const toggleNodeOpen = (id: number) => {
    setOpenNodes((prev) => ({ ...prev, [id]: !prev[id] }));
  };

  return (
    <div className="p-4  flex flex-col gap-4">
      <div>
        <Button
          variant="default"
          className="text-xs"
          size="sm"
          onClick={() => onNewConversation()}
        >
          + Start New Conversation
        </Button>
      </div>
      <div className="flex flex-col gap-1">
        {nodes.map((node) => (
          <ConversationTreeItem
            node={node}
            key={node.id}
            isOpen={!!openNodes[node.id]}
            openNodes={openNodes}
            toggleNodeOpen={toggleNodeOpen}
            onToggleOpen={() => toggleNodeOpen(node.id)}
            onSelectConversation={onSelectConversation}
            activeConversationId={activeConversationId}
          />
        ))}
      </div>
    </div>
  );
}

function convertConversationsToNodes(convos: Conversation[]): Node[] {
  let nodesArr: Node[] = [];

  for (const convo of convos) {
    const node: Node = {
      id: convo.id,
      parentId: convo.parent_conversation_id,
      name: convo.title,
      nodes: [],
    };

    if (!convo.parent_conversation_id) {
      // This is a root node
      nodesArr.push(node);
    } else {
      // Find parent node recursively
      const parent = findNodeById(nodesArr, convo.parent_conversation_id);
      if (parent) {
        if (!parent.nodes) parent.nodes = [];
        parent.nodes.push(node);
      }
    }
  }

  return nodesArr;
}

function findNodeById(nodes: Node[], id: number): Node | undefined {
  for (const node of nodes) {
    if (node.id === id) return node;
    if (node.nodes) {
      const found = findNodeById(node.nodes, id);
      if (found) return found;
    }
  }
  return undefined;
}

function hasActiveDescendant(node: Node, activeId: number): boolean {
  if (node.id === activeId) return true;
  return (
    node.nodes?.some((child) => hasActiveDescendant(child, activeId)) || false
  );
}
