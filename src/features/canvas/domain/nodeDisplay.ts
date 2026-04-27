import {
  CANVAS_NODE_TYPES,
  type CanvasNodeData,
  type CanvasNodeType,
  type ExportImageNodeResultKind,
} from './canvasNodes';

export const DEFAULT_NODE_DISPLAY_NAME: Record<CanvasNodeType, string> = {
  [CANVAS_NODE_TYPES.upload]: '上传图片',
  [CANVAS_NODE_TYPES.imageEdit]: 'AI 图片',
  [CANVAS_NODE_TYPES.exportImage]: '结果图片',
  [CANVAS_NODE_TYPES.textAnnotation]: '文本注释',
  [CANVAS_NODE_TYPES.group]: '分组',
  [CANVAS_NODE_TYPES.storyboardSplit]: '切割结果',
  [CANVAS_NODE_TYPES.storyboardGen]: '分镜生成',
};

export const DEFAULT_NODE_DISPLAY_NAME_KEY: Record<CanvasNodeType, string> = {
  [CANVAS_NODE_TYPES.upload]: 'node.defaults.upload',
  [CANVAS_NODE_TYPES.imageEdit]: 'node.defaults.imageEdit',
  [CANVAS_NODE_TYPES.exportImage]: 'node.defaults.exportImage.generic',
  [CANVAS_NODE_TYPES.textAnnotation]: 'node.defaults.textAnnotation',
  [CANVAS_NODE_TYPES.group]: 'node.defaults.group',
  [CANVAS_NODE_TYPES.storyboardSplit]: 'node.defaults.storyboardSplit',
  [CANVAS_NODE_TYPES.storyboardGen]: 'node.defaults.storyboardGen',
};

export const EXPORT_RESULT_DISPLAY_NAME: Record<ExportImageNodeResultKind, string> = {
  generic: '结果图片',
  storyboardGenOutput: '分镜输出',
  storyboardSplitExport: '切割导出',
  storyboardFrameEdit: '分镜帧',
};

export const EXPORT_RESULT_DISPLAY_NAME_KEY: Record<ExportImageNodeResultKind, string> = {
  generic: 'node.defaults.exportImage.generic',
  storyboardGenOutput: 'node.defaults.exportImage.storyboardGenOutput',
  storyboardSplitExport: 'node.defaults.exportImage.storyboardSplitExport',
  storyboardFrameEdit: 'node.defaults.exportImage.storyboardFrameEdit',
};

export type NodeDisplayTranslator = (key: string) => string;

function resolveExportResultDefault(data: Partial<CanvasNodeData>): string {
  const resultKind = (data as { resultKind?: ExportImageNodeResultKind }).resultKind ?? 'generic';
  return EXPORT_RESULT_DISPLAY_NAME[resultKind];
}

function resolveExportResultDefaultKey(data: Partial<CanvasNodeData>): string {
  const resultKind = (data as { resultKind?: ExportImageNodeResultKind }).resultKind ?? 'generic';
  return EXPORT_RESULT_DISPLAY_NAME_KEY[resultKind];
}

export function getDefaultNodeDisplayName(type: CanvasNodeType, data: Partial<CanvasNodeData>): string {
  if (type === CANVAS_NODE_TYPES.exportImage) {
    return resolveExportResultDefault(data);
  }
  return DEFAULT_NODE_DISPLAY_NAME[type];
}

export function getDefaultNodeDisplayNameKey(
  type: CanvasNodeType,
  data: Partial<CanvasNodeData>
): string {
  if (type === CANVAS_NODE_TYPES.exportImage) {
    return resolveExportResultDefaultKey(data);
  }
  return DEFAULT_NODE_DISPLAY_NAME_KEY[type];
}

export function isLegacyDefaultDisplayName(
  type: CanvasNodeType,
  data: Partial<CanvasNodeData>,
  value: string
): boolean {
  const normalizedValue = value.trim();
  if (!normalizedValue) {
    return false;
  }
  return normalizedValue === getDefaultNodeDisplayName(type, data);
}

export function resolveNodeDisplayName(
  type: CanvasNodeType,
  data: Partial<CanvasNodeData>,
  translate?: NodeDisplayTranslator
): string {
  const customTitle = typeof data.displayName === 'string' ? data.displayName.trim() : '';
  if (customTitle && !isLegacyDefaultDisplayName(type, data, customTitle)) {
    return customTitle;
  }

  if (type === CANVAS_NODE_TYPES.group) {
    const legacyLabel = typeof (data as { label?: string }).label === 'string'
      ? (data as { label?: string }).label?.trim()
      : '';
    if (legacyLabel && !isLegacyDefaultDisplayName(type, data, legacyLabel)) {
      return legacyLabel;
    }
  }

  if (translate) {
    return translate(getDefaultNodeDisplayNameKey(type, data));
  }

  return getDefaultNodeDisplayName(type, data);
}

export function isNodeUsingDefaultDisplayName(type: CanvasNodeType, data: Partial<CanvasNodeData>): boolean {
  const customTitle = typeof data.displayName === 'string' ? data.displayName.trim() : '';
  if (!customTitle) {
    return true;
  }
  return customTitle === getDefaultNodeDisplayName(type, data);
}
