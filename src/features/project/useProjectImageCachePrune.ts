import { useEffect } from 'react';

import { pruneProjectImageCache } from '@/commands/projectState';

const DEFAULT_PRUNE_DELAY_MS = 8_000;
const DEFAULT_PRUNE_MIN_AGE_MS = 24 * 60 * 60 * 1000;

let hasScheduledProjectImageCachePrune = false;

function scheduleIdleTask(callback: () => void, timeoutMs: number): () => void {
  if (typeof window === 'undefined') {
    return () => {};
  }

  const requestIdleCallback = window.requestIdleCallback;
  const cancelIdleCallback = window.cancelIdleCallback;

  if (requestIdleCallback && cancelIdleCallback) {
    const idleId = requestIdleCallback(callback, { timeout: timeoutMs });
    return () => cancelIdleCallback(idleId);
  }

  const timerId = window.setTimeout(callback, timeoutMs);
  return () => window.clearTimeout(timerId);
}

export function useProjectImageCachePrune(isReady: boolean): void {
  useEffect(() => {
    if (!isReady || hasScheduledProjectImageCachePrune) {
      return;
    }

    hasScheduledProjectImageCachePrune = true;
    let cancelled = false;
    const cancelIdleTask = scheduleIdleTask(() => {
      if (cancelled) {
        return;
      }

      void pruneProjectImageCache(DEFAULT_PRUNE_MIN_AGE_MS).catch((error) => {
        console.warn('failed to prune project image cache', error);
      });
    }, DEFAULT_PRUNE_DELAY_MS);

    return () => {
      cancelled = true;
      cancelIdleTask();
    };
  }, [isReady]);
}
