import { describe, expect, it } from 'vitest';

import { CANVAS_NODE_TYPES, type CanvasNode } from '@/features/canvas/domain/canvasNodes';
import { fromProjectRecord, toProjectRecord, type Project } from './projectStore';

function makeImageNode(id: string, imageUrl: string): CanvasNode {
  return {
    id,
    type: CANVAS_NODE_TYPES.upload,
    position: { x: 0, y: 0 },
    data: {
      displayName: '上传图片',
      imageUrl,
      previewImageUrl: `${imageUrl}:preview`,
      aspectRatio: '1:1',
    },
  };
}

function makeProject(historyImageCount: number): Project {
  const nodes = [makeImageNode('current', 'current-image')];
  return {
    id: 'project-1',
    name: 'Project',
    createdAt: 1,
    updatedAt: 2,
    nodeCount: nodes.length,
    nodes,
    edges: [],
    viewport: { x: 0, y: 0, zoom: 1 },
    history: {
      past: Array.from({ length: historyImageCount }, (_, index) => ({
        nodes: [makeImageNode(`past-${index}`, `history-image-${index}`)],
        edges: [],
      })),
      future: [],
    },
  };
}

describe('project persistence image pool', () => {
  it('trims history before encoding image references', () => {
    const record = toProjectRecord(makeProject(14));
    const historyPayload = JSON.parse(record.historyJson) as { imagePool: string[] };

    expect(historyPayload.imagePool).toContain('current-image');
    expect(historyPayload.imagePool).toContain('history-image-2');
    expect(historyPayload.imagePool).not.toContain('history-image-0');
    expect(historyPayload.imagePool).not.toContain('history-image-1');
  });

  it('decodes legacy imagePool references from persisted records', () => {
    const record = {
      id: 'project-legacy',
      name: 'Legacy',
      createdAt: 1,
      updatedAt: 2,
      nodeCount: 1,
      nodesJson: JSON.stringify([
        {
          id: 'node-1',
          type: CANVAS_NODE_TYPES.upload,
          position: { x: 0, y: 0 },
          data: {
            imageUrl: '__img_ref__:0',
            previewImageUrl: '__img_ref__:1',
            aspectRatio: '1:1',
          },
        },
      ]),
      edgesJson: '[]',
      viewportJson: JSON.stringify({ x: 0, y: 0, zoom: 1 }),
      historyJson: JSON.stringify({
        past: [],
        future: [],
        imagePool: ['original-path', 'preview-path'],
      }),
    };

    const project = fromProjectRecord(record);
    const data = project.nodes[0]?.data as { imageUrl?: string | null; previewImageUrl?: string | null };

    expect(data.imageUrl).toBe('original-path');
    expect(data.previewImageUrl).toBe('preview-path');
  });
});
