import type { CanvasEdge, CanvasNode } from '../domain/canvasNodes';
import { selectInputImagesForNode } from '../state/canvasSelectors';
import type { GraphImageResolver } from './ports';

export class DefaultGraphImageResolver implements GraphImageResolver {
  collectInputImages(nodeId: string, nodes: CanvasNode[], edges: CanvasEdge[]): string[] {
    return selectInputImagesForNode(nodeId, nodes, edges);
  }
}
