import {
  generateImage,
  getGenerateImageJob,
  setApiKey,
  submitGenerateImageJob,
} from '@/commands/ai';
import {
  createPreviewDataUrl,
  imageUrlToDataUrl,
  persistImageLocally,
} from '@/features/canvas/application/imageData';
import { QIANHAI_IMAGE_REQUEST_MODELS } from '@/features/canvas/models/image/qianhai/shared';

import type { AiGateway, GenerateImagePayload } from '../application/ports';

const QIANHAI_REFERENCE_IMAGE_MAX_DIMENSION = 1024;

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

async function normalizeReferenceImages(payload: GenerateImagePayload): Promise<string[] | undefined> {
  const isKieModel = payload.model.startsWith('kie/');
  const isFalModel = payload.model.startsWith('fal/');
  const isQianhaiModel = isQianhaiRequestModel(payload.model);
  return payload.referenceImages
    ? await Promise.all(
      payload.referenceImages.map(async (imageUrl) =>
        isKieModel || isFalModel
          ? await imageUrlToDataUrl(imageUrl)
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
