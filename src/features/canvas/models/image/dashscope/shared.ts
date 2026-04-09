import { normalizeImageSizeValue } from '@/features/canvas/application/generationSize';

import type { ImageModelDefinition } from '../../types';

export const DASHSCOPE_PROVIDER_ID = 'dashscope';
export const DASHSCOPE_QWEN_MAX_REFERENCE_IMAGES = 3;
export const DASHSCOPE_QWEN_IMAGE_REQUEST_MODELS = [
  'qwen-image-2.0-pro',
  'qwen-image-2.0',
] as const;

export const DASHSCOPE_QWEN_IMAGE_RESOLUTIONS = [
  '1024*1024',
  '1536*1536',
  '768*1152',
  '1024*1536',
  '1152*768',
  '1536*1024',
  '960*1280',
  '1080*1440',
  '1280*960',
  '1440*1080',
  '720*1280',
  '1080*1920',
  '1280*720',
  '1920*1080',
  '1344*576',
  '2048*872',
] as const;

export const DASHSCOPE_QWEN_IMAGE_ASPECT_RATIOS = [
  '1:1',
  '2:3',
  '3:2',
  '3:4',
  '4:3',
  '9:16',
  '16:9',
  '21:9',
] as const;

interface CreateDashScopeQwenImageModelOptions {
  id: string;
  displayName: string;
  description: string;
  requestModel: string;
}

export function normalizeDashScopeQwenResolution(requestedResolution: string): string {
  const normalizedValue = normalizeImageSizeValue(requestedResolution);
  return normalizedValue || requestedResolution.trim();
}

export function createDashScopeQwenImageModel({
  id,
  displayName,
  description,
  requestModel,
}: CreateDashScopeQwenImageModelOptions): ImageModelDefinition {
  return {
    id,
    mediaType: 'image',
    displayName,
    providerId: DASHSCOPE_PROVIDER_ID,
    description,
    eta: '1min',
    expectedDurationMs: 60000,
    defaultAspectRatio: '1:1',
    defaultResolution: '1024*1024',
    resolutionControlMode: 'sizeOnly',
    maxReferenceImages: DASHSCOPE_QWEN_MAX_REFERENCE_IMAGES,
    aspectRatios: DASHSCOPE_QWEN_IMAGE_ASPECT_RATIOS.map((value) => ({
      value,
      label: value,
    })),
    resolutions: DASHSCOPE_QWEN_IMAGE_RESOLUTIONS.map((value) => ({
      value,
      label: value,
    })),
    normalizeRequestedResolution: (requestedResolution) =>
      normalizeDashScopeQwenResolution(requestedResolution),
    resolveRequest: ({ referenceImageCount }) => ({
      requestModel,
      modeLabel: referenceImageCount > 0 ? '编辑模式' : '生成模式',
    }),
  };
}
