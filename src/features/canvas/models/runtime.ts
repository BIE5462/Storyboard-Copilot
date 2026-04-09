import type { ProviderApiKeys } from '@/stores/settingsStore';

import {
  isQianhaiGrokModel,
  QIANHAI_GROK_CREDENTIAL_KEY,
  QIANHAI_GROK_PROVIDER_ROUTE,
  QIANHAI_PROVIDER_ID,
} from './image/qianhai/runtime';

export function getProviderCredentialKeyForModel(modelId: string, providerId: string): string {
  if (providerId === QIANHAI_PROVIDER_ID && isQianhaiGrokModel(modelId)) {
    return QIANHAI_GROK_CREDENTIAL_KEY;
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
