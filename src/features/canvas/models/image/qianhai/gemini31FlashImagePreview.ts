import { createQianhaiImagePreviewModel } from './shared';

export const QIANHAI_GEMINI_3_1_FLASH_IMAGE_PREVIEW_MODEL_ID =
  'qianhai/gemini-3.1-flash-image-preview';

export const imageModel = createQianhaiImagePreviewModel({
  id: QIANHAI_GEMINI_3_1_FLASH_IMAGE_PREVIEW_MODEL_ID,
  displayName: 'Gemini 3.1 Flash Image Preview',
  description: '千海 · Gemini 3.1 Flash Image Preview 图像生成与编辑',
  requestModel: 'gemini-3.1-flash-image-preview',
});
