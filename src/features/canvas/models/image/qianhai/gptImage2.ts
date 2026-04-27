import {
  createQianhaiImagePreviewModel,
  QIANHAI_GPT_IMAGE_ASPECT_RATIOS,
  QIANHAI_GPT_IMAGE_RESOLUTIONS,
} from './shared';

export const QIANHAI_GPT_IMAGE_2_MODEL_ID = 'qianhai/gpt-image-2';

export const imageModel = createQianhaiImagePreviewModel({
  id: QIANHAI_GPT_IMAGE_2_MODEL_ID,
  displayName: 'GPT Image 2',
  description: '千海 · GPT Image 2 图像生成与参考图编辑',
  requestModel: 'gpt-image-2',
  aspectRatios: QIANHAI_GPT_IMAGE_ASPECT_RATIOS,
  resolutions: QIANHAI_GPT_IMAGE_RESOLUTIONS,
  defaultAspectRatio: '1:1',
  defaultResolution: '1024x1024',
  resolutionControlMode: 'sizeOnly',
  maxReferenceImages: 10,
});
