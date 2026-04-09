import {
  generateImage,
  getGenerateImageJob,
  setApiKey,
  submitGenerateImageJob,
} from '@/commands/ai';
import {
  createPreviewDataUrl,
  extractBase64Payload,
  imageUrlToDataUrl,
  persistImageLocally,
} from '@/features/canvas/application/imageData';
import { DASHSCOPE_QWEN_IMAGE_REQUEST_MODELS } from '@/features/canvas/models/image/dashscope/shared';
import { QIANHAI_IMAGE_REQUEST_MODELS } from '@/features/canvas/models/image/qianhai/shared';

import type { AiGateway, GenerateImagePayload } from '../application/ports';

const QIANHAI_REFERENCE_IMAGE_MAX_DIMENSION = 1024;
const DASHSCOPE_REFERENCE_IMAGE_MAX_BYTES = 10 * 1024 * 1024;
const DASHSCOPE_REFERENCE_IMAGE_DIMENSION_CANDIDATES = [
  2048,
  1792,
  1536,
  1280,
  1024,
  768,
  512,
  384,
] as const;

async function normalizeQianhaiReferenceImage(imageUrl: string): Promise<string> {
  try {
    return await createPreviewDataUrl(imageUrl, QIANHAI_REFERENCE_IMAGE_MAX_DIMENSION);
  } catch (error) {
    console.warn('[tauriAiGateway] Failed to create preview-sized qianhai reference image', {
      imageUrl,
      error,
    });
    return await imageUrlToDataUrl(imageUrl);
  }
}

function isQianhaiRequestModel(model: string): boolean {
  const trimmedModel = model.trim();
  if (!trimmedModel) {
    return false;
  }

  if (trimmedModel.startsWith('qianhai/')) {
    return true;
  }

  const normalizedModel = trimmedModel.split('/').pop() ?? trimmedModel;
  return QIANHAI_IMAGE_REQUEST_MODELS.includes(
    normalizedModel as (typeof QIANHAI_IMAGE_REQUEST_MODELS)[number]
  );
}

function isDashScopeRequestModel(model: string): boolean {
  const trimmedModel = model.trim();
  if (!trimmedModel) {
    return false;
  }

  if (trimmedModel.startsWith('dashscope/')) {
    return true;
  }

  const normalizedModel = trimmedModel.split('/').pop() ?? trimmedModel;
  return DASHSCOPE_QWEN_IMAGE_REQUEST_MODELS.includes(
    normalizedModel as (typeof DASHSCOPE_QWEN_IMAGE_REQUEST_MODELS)[number]
  );
}

function estimateDataUrlByteSize(dataUrl: string): number {
  const payload = extractBase64Payload(dataUrl);
  if (!payload) {
    return 0;
  }

  const paddingLength = payload.endsWith('==') ? 2 : payload.endsWith('=') ? 1 : 0;
  return Math.floor((payload.length * 3) / 4) - paddingLength;
}

async function normalizeDashScopeReferenceImage(imageUrl: string): Promise<string> {
  const sourceDataUrl = await imageUrlToDataUrl(imageUrl);

  for (const maxDimension of DASHSCOPE_REFERENCE_IMAGE_DIMENSION_CANDIDATES) {
    const candidate = await createPreviewDataUrl(sourceDataUrl, maxDimension);
    if (
      candidate.startsWith('data:image/')
      && estimateDataUrlByteSize(candidate) <= DASHSCOPE_REFERENCE_IMAGE_MAX_BYTES
    ) {
      return candidate;
    }
  }

  const originalBytes = estimateDataUrlByteSize(sourceDataUrl);
  if (
    sourceDataUrl.startsWith('data:image/')
    && originalBytes > 0
    && originalBytes <= DASHSCOPE_REFERENCE_IMAGE_MAX_BYTES
  ) {
    return sourceDataUrl;
  }

  throw new Error(
    `DashScope reference image exceeds 10MB after normalization: ${Math.round(originalBytes / 1024)}KB`
  );
}

async function normalizeReferenceImages(payload: GenerateImagePayload): Promise<string[] | undefined> {
  const isKieModel = payload.model.startsWith('kie/');
  const isFalModel = payload.model.startsWith('fal/');
  const isQianhaiModel = isQianhaiRequestModel(payload.model);
  const isDashScopeModel = isDashScopeRequestModel(payload.model);
  return payload.referenceImages
    ? await Promise.all(
      payload.referenceImages.map(async (imageUrl) =>
        isKieModel || isFalModel
          ? await imageUrlToDataUrl(imageUrl)
          : isDashScopeModel
            ? await normalizeDashScopeReferenceImage(imageUrl)
          : isQianhaiModel
            ? await normalizeQianhaiReferenceImage(imageUrl)
          : await persistImageLocally(imageUrl)
      )
    )
    : undefined;
}

export const tauriAiGateway: AiGateway = {
  setApiKey,
  generateImage: async (payload: GenerateImagePayload) => {
    const normalizedReferenceImages = await normalizeReferenceImages(payload);

    return await generateImage({
      prompt: payload.prompt,
      model: payload.model,
      size: payload.size,
      aspect_ratio: payload.aspectRatio,
      reference_images: normalizedReferenceImages,
      extra_params: payload.extraParams,
    });
  },
  submitGenerateImageJob: async (payload: GenerateImagePayload) => {
    const normalizedReferenceImages = await normalizeReferenceImages(payload);
    return await submitGenerateImageJob({
      prompt: payload.prompt,
      model: payload.model,
      size: payload.size,
      aspect_ratio: payload.aspectRatio,
      reference_images: normalizedReferenceImages,
      extra_params: payload.extraParams,
    });
  },
  getGenerateImageJob,
};
