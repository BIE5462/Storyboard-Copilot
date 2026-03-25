import { createQianhaiImagePreviewModel } from './shared';

export const QIANHAI_GEMINI_3_PRO_IMAGE_PREVIEW_MODEL_ID =
  'qianhai/gemini-3-pro-image-preview';

export const imageModel = createQianhaiImagePreviewModel({
  id: QIANHAI_GEMINI_3_PRO_IMAGE_PREVIEW_MODEL_ID,
  displayName: 'nano banana pro',
  description: '千海 · Gemini 3 Pro Image Preview 图像生成与编辑',
  requestModel: 'gemini-3-pro-image-preview',
});
