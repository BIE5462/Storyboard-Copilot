import {
  createQianhaiImagePreviewModel,
  QIANHAI_REFERENCE_IMAGE_ASPECT_RATIOS,
  QIANHAI_REFERENCE_IMAGE_RESOLUTIONS,
} from './shared';

export const QIANHAI_GROK_IMAGE_MODEL_ID = 'qianhai/grok-image';

export const imageModel = createQianhaiImagePreviewModel({
  id: QIANHAI_GROK_IMAGE_MODEL_ID,
  displayName: 'Grok 图像',
  description: '千海 · Grok 图像生成与参考图编辑',
  requestModel: QIANHAI_GROK_IMAGE_MODEL_ID,
  aspectRatios: QIANHAI_REFERENCE_IMAGE_ASPECT_RATIOS,
  resolutions: QIANHAI_REFERENCE_IMAGE_RESOLUTIONS,
  defaultAspectRatio: '1:1',
  defaultResolution: '1024x1024',
});
