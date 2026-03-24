import type { ImageModelDefinition } from '../../types';

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

interface CreateQianhaiImageModelOptions {
  id: string;
  displayName: string;
  description: string;
  requestModel: string;
}

export function createQianhaiImagePreviewModel({
  id,
  displayName,
  description,
  requestModel,
}: CreateQianhaiImageModelOptions): ImageModelDefinition {
  return {
    id,
    mediaType: 'image',
    displayName,
    providerId: 'qianhai',
    description,
    eta: '1min',
    expectedDurationMs: 60000,
    defaultAspectRatio: '1:1',
    defaultResolution: '2K',
    aspectRatios: QIANHAI_IMAGE_ASPECT_RATIOS.map((value) => ({
      value,
      label: value,
    })),
    resolutions: QIANHAI_IMAGE_RESOLUTIONS.map((value) => ({
      value,
      label: value,
    })),
    resolveRequest: ({ referenceImageCount }) => ({
      requestModel,
      modeLabel: referenceImageCount > 0 ? '编辑模式' : '生成模式',
    }),
  };
}
