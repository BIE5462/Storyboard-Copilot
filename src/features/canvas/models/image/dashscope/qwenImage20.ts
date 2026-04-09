import { createDashScopeQwenImageModel } from './shared';

export const DASHSCOPE_QWEN_IMAGE_20_MODEL_ID = 'dashscope/qwen-image-2.0';

export const imageModel = createDashScopeQwenImageModel({
  id: DASHSCOPE_QWEN_IMAGE_20_MODEL_ID,
  displayName: 'Qwen Image 2.0',
  description: 'DashScope · Qwen Image 2.0 图像生成与参考图编辑',
  requestModel: DASHSCOPE_QWEN_IMAGE_20_MODEL_ID,
});
