import { QIANHAI_GROK_IMAGE_MODEL_ID } from './grokImage';

export const QIANHAI_PROVIDER_ID = 'qianhai';
export const QIANHAI_GROK_CREDENTIAL_KEY = 'qianhai_grok';
export const QIANHAI_GROK_PROVIDER_ROUTE = 'qianhai-grok';
export const QIANHAI_MODEL_NAME_EXTRA_PARAM_KEY = 'qianhai_model_name';

export type QianhaiCredentialKey =
  | typeof QIANHAI_PROVIDER_ID
  | typeof QIANHAI_GROK_CREDENTIAL_KEY;

export function isQianhaiGrokModel(modelId: string): boolean {
  return modelId === QIANHAI_GROK_IMAGE_MODEL_ID || modelId === 'grok-image';
}

export function resolveConfiguredQianhaiRequestModelName(
  modelId: string,
  grokModelName: string
): string {
  if (isQianhaiGrokModel(modelId)) {
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
