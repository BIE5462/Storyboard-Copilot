import type { ProviderApiKeys } from '@/stores/settingsStore';

import {
  isQianhaiGptImage2AllModel,
  isQianhaiGrokModel,
  QIANHAI_GPT_IMAGE_2_ALL_CREDENTIAL_KEY,
  QIANHAI_GPT_IMAGE_2_ALL_PROVIDER_ROUTE,
  QIANHAI_GROK_CREDENTIAL_KEY,
  QIANHAI_GROK_PROVIDER_ROUTE,
  QIANHAI_PROVIDER_ID,
} from './image/qianhai/runtime';

export function getProviderCredentialKeyForModel(modelId: string, providerId: string): string {
  if (providerId === QIANHAI_PROVIDER_ID && isQianhaiGrokModel(modelId)) {
    return QIANHAI_GROK_CREDENTIAL_KEY;
  }

  if (providerId === QIANHAI_PROVIDER_ID && isQianhaiGptImage2AllModel(modelId)) {
    return QIANHAI_GPT_IMAGE_2_ALL_CREDENTIAL_KEY;
  }

  return providerId;
}

export function getProviderRouteForCredentialKey(
  credentialKey: string,
  providerId: string
): string {
  if (providerId === QIANHAI_PROVIDER_ID && credentialKey === QIANHAI_GROK_CREDENTIAL_KEY) {
    return QIANHAI_GROK_PROVIDER_ROUTE;
  }

  if (
    providerId === QIANHAI_PROVIDER_ID &&
    credentialKey === QIANHAI_GPT_IMAGE_2_ALL_CREDENTIAL_KEY
  ) {
    return QIANHAI_GPT_IMAGE_2_ALL_PROVIDER_ROUTE;
  }

  return providerId;
}

export function getProviderApiKeyForModel(
  apiKeys: ProviderApiKeys,
  modelId: string,
  providerId: string
): string {
  const credentialKey = getProviderCredentialKeyForModel(modelId, providerId);
  return apiKeys[credentialKey] ?? '';
}
