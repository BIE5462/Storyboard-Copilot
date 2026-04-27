import type { ImageModelDefinition, ResolutionControlMode } from '../../types';

export const QIANHAI_IMAGE_ASPECT_RATIOS = [
  '1:1',
  '1:4',
  '1:8',
  '9:16',
  '16:9',
  '3:4',
  '4:3',
  '4:1',
  '8:1',
  '2:3',
  '3:2',
  '5:4',
  '4:5',
  '21:9',
] as const;

export const QIANHAI_IMAGE_RESOLUTIONS = ['0.5K', '1K', '2K', '4K'] as const;
export const QIANHAI_REFERENCE_IMAGE_RESOLUTIONS = ['1024x1024'] as const;
export const QIANHAI_REFERENCE_IMAGE_ASPECT_RATIOS = ['1:1'] as const;
export const QIANHAI_GPT_IMAGE_RESOLUTIONS = [
  '1024x1024',
  '1024x768',
  '1024x576',
  '768x1024',
  '576x1024',
  '2048x2048',
  '2048x1536',
  '2048x1152',
  '1536x2048',
  '1152x2048',
  '3840x3840',
  '3840x2160',
  '2160x3840',
  'auto',
] as const;
export const QIANHAI_GPT_IMAGE_ASPECT_RATIOS = ['1:1', '4:3', '16:9', '3:4', '9:16'] as const;
export const QIANHAI_IMAGE_REQUEST_MODELS = [
  'gemini-3.1-flash-image-preview',
  'gemini-3-pro-image-preview',
  'gpt-image-2',
  'gpt-image-2-all',
] as const;

interface CreateQianhaiImageModelOptions {
  id: string;
  displayName: string;
  description: string;
  requestModel: string;
  aspectRatios?: readonly string[];
  resolutions?: readonly string[];
  defaultAspectRatio?: string;
  defaultResolution?: string;
  resolutionControlMode?: ResolutionControlMode;
  maxReferenceImages?: number;
  normalizeRequestedResolution?: (requestedResolution: string) => string;
}

export function createQianhaiImagePreviewModel({
  id,
  displayName,
  description,
  requestModel,
  aspectRatios = QIANHAI_IMAGE_ASPECT_RATIOS,
  resolutions = QIANHAI_IMAGE_RESOLUTIONS,
  defaultAspectRatio = aspectRatios[0] ?? '1:1',
  defaultResolution = resolutions[0] ?? '2K',
  resolutionControlMode = 'paired',
  maxReferenceImages,
  normalizeRequestedResolution,
}: CreateQianhaiImageModelOptions): ImageModelDefinition {
  return {
    id,
    mediaType: 'image',
    displayName,
    providerId: 'qianhai',
    description,
    eta: '1min',
    expectedDurationMs: 60000,
    defaultAspectRatio,
    defaultResolution,
    resolutionControlMode,
    maxReferenceImages,
    aspectRatios: aspectRatios.map((value) => ({
      value,
      label: value,
    })),
    resolutions: resolutions.map((value) => ({
      value,
      label: value,
    })),
    normalizeRequestedResolution: normalizeRequestedResolution
      ? (requestedResolution) => normalizeRequestedResolution(requestedResolution)
      : undefined,
    resolveRequest: ({ referenceImageCount }) => ({
      requestModel,
      modeLabel: referenceImageCount > 0 ? '编辑模式' : '生成模式',
    }),
  };
}
