import { describe, expect, it } from 'vitest';

import {
  CANVAS_NODE_TYPES,
  type CanvasEdge,
  type CanvasNode,
} from '@/features/canvas/domain/canvasNodes';
import { selectInputImagesForNode } from './canvasSelectors';

function uploadNode(id: string, imageUrl: string, previewImageUrl?: string): CanvasNode {
  return {
    id,
    type: CANVAS_NODE_TYPES.upload,
    position: { x: 0, y: 0 },
    data: {
      imageUrl,
      previewImageUrl: previewImageUrl ?? null,
      aspectRatio: '1:1',
    },
  };
}

function textNode(id: string): CanvasNode {
  return {
    id,
    type: CANVAS_NODE_TYPES.textAnnotation,
    position: { x: 0, y: 0 },
    data: {
      content: '',
    },
  };
}

const targetNode: CanvasNode = {
  id: 'target',
  type: CANVAS_NODE_TYPES.imageEdit,
  position: { x: 0, y: 0 },
  data: {
    imageUrl: null,
    previewImageUrl: null,
    aspectRatio: '1:1',
    prompt: '',
    model: 'model',
    size: '1K',
  },
};

function edge(source: string, target = 'target'): CanvasEdge {
  return {
    id: `${source}-${target}`,
    source,
    target,
  };
}

describe('canvas input image selectors', () => {
  it('returns connected image sources in edge order', () => {
    const nodes = [uploadNode('a', 'image-a'), uploadNode('b', 'image-b'), targetNode];
    const edges = [edge('a'), edge('b')];

    expect(selectInputImagesForNode('target', nodes, edges)).toEqual(['image-a', 'image-b']);
  });

  it('deduplicates duplicate images and ignores non-image source nodes', () => {
    const nodes = [
      uploadNode('a', 'same-image'),
      uploadNode('b', 'same-image'),
      textNode('text'),
      targetNode,
    ];
    const edges = [edge('a'), edge('b'), edge('text')];

    expect(selectInputImagesForNode('target', nodes, edges)).toEqual(['same-image']);
  });

  it('ignores edges whose source node was deleted', () => {
    const nodes = [uploadNode('a', 'image-a'), targetNode];
    const edges = [edge('a'), edge('deleted')];

    expect(selectInputImagesForNode('target', nodes, edges)).toEqual(['image-a']);
  });
});
