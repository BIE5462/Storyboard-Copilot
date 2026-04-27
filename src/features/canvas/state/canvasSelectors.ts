import { useMemo } from 'react';

import {
  isExportImageNode,
  isImageEditNode,
  isUploadNode,
  type CanvasEdge,
  type CanvasNode,
} from '@/features/canvas/domain/canvasNodes';
import { resolveActionImageSource } from '@/features/canvas/application/imageData';
import { useCanvasStore } from '@/stores/canvasStore';

export interface InputImageRef {
  imageUrl: string;
  previewImageUrl: string | null;
}

export function selectCanvasNodeById(nodes: CanvasNode[], nodeId: string): CanvasNode | null {
  return nodes.find((node) => node.id === nodeId) ?? null;
}

export function selectInputImageRefsForNode(
  nodeId: string,
  nodes: CanvasNode[],
  edges: CanvasEdge[]
): InputImageRef[] {
  const nodeById = new Map(nodes.map((node) => [node.id, node] as const));
  const dedupedByImageUrl = new Map<string, InputImageRef>();

  for (const edge of edges) {
    if (edge.target !== nodeId) {
      continue;
    }

    const sourceNode = nodeById.get(edge.source);
    if (!isUploadNode(sourceNode) && !isImageEditNode(sourceNode) && !isExportImageNode(sourceNode)) {
      continue;
    }

    const imageUrl = resolveActionImageSource(
      sourceNode.data.imageUrl,
      sourceNode.data.previewImageUrl
    );
    if (!imageUrl || dedupedByImageUrl.has(imageUrl)) {
      continue;
    }

    dedupedByImageUrl.set(imageUrl, {
      imageUrl,
      previewImageUrl: sourceNode.data.previewImageUrl ?? null,
    });
  }

  return Array.from(dedupedByImageUrl.values());
}

export function selectInputImagesForNode(
  nodeId: string,
  nodes: CanvasNode[],
  edges: CanvasEdge[]
): string[] {
  return selectInputImageRefsForNode(nodeId, nodes, edges).map((item) => item.imageUrl);
}

export function useCanvasNodeById(nodeId: string): CanvasNode | null {
  return useCanvasStore(
    (state) => state.nodes.find((node) => node.id === nodeId) ?? null
  );
}

export function useCanvasInputImageRefs(nodeId: string): InputImageRef[] {
  const nodes = useCanvasStore((state) => state.nodes);
  const edges = useCanvasStore((state) => state.edges);
  return useMemo(() => selectInputImageRefsForNode(nodeId, nodes, edges), [edges, nodeId, nodes]);
}

export function useCanvasInputImages(nodeId: string): string[] {
  const inputImageRefs = useCanvasInputImageRefs(nodeId);
  return useMemo(() => inputImageRefs.map((item) => item.imageUrl), [inputImageRefs]);
}
