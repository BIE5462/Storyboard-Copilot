import { QIANHAI_GROK_IMAGE_MODEL_ID } from './grokImage';
import { QIANHAI_GPT_IMAGE_2_ALL_MODEL_ID } from './gptImage2All';

export const QIANHAI_PROVIDER_ID = 'qianhai';
export const QIANHAI_GROK_CREDENTIAL_KEY = 'qianhai_grok';
export const QIANHAI_GROK_PROVIDER_ROUTE = 'qianhai-grok';
export const QIANHAI_GPT_IMAGE_2_ALL_CREDENTIAL_KEY = 'qianhai_gpt_image_2_all';
export const QIANHAI_GPT_IMAGE_2_ALL_PROVIDER_ROUTE = 'qianhai-gpt-image-2-all';
export const QIANHAI_MODEL_NAME_EXTRA_PARAM_KEY = 'qianhai_model_name';

export type QianhaiCredentialKey =
  | typeof QIANHAI_PROVIDER_ID
  | typeof QIANHAI_GROK_CREDENTIAL_KEY
  | typeof QIANHAI_GPT_IMAGE_2_ALL_CREDENTIAL_KEY;

export function isQianhaiGrokModel(modelId: string): boolean {
  return modelId === QIANHAI_GROK_IMAGE_MODEL_ID || modelId === 'grok-image';
}

export function isQianhaiGptImage2AllModel(modelId: string): boolean {
  return modelId === QIANHAI_GPT_IMAGE_2_ALL_MODEL_ID || modelId === 'gpt-image-2-all';
}

export function requiresConfiguredQianhaiModelName(modelId: string): boolean {
  return isQianhaiGrokModel(modelId);
}

export function resolveConfiguredQianhaiRequestModelName(
  modelId: string,
  grokModelName: string
): string {
  if (requiresConfiguredQianhaiModelName(modelId)) {
    return grokModelName.trim();
  }

  return '';
}

export function attachConfiguredQianhaiModelName(
  extraParams: Record<string, unknown>,
  modelId: string,
  grokModelName: string
): Record<string, unknown> {
  const configuredModelName = resolveConfiguredQianhaiRequestModelName(modelId, grokModelName);
  if (!configuredModelName) {
    return extraParams;
  }

  return {
    ...extraParams,
    [QIANHAI_MODEL_NAME_EXTRA_PARAM_KEY]: configuredModelName,
  };
}
