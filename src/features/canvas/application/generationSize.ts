import { DEFAULT_ASPECT_RATIO } from '@/features/canvas/domain/canvasNodes';

import { parseAspectRatio } from './imageData';

export interface ParsedExactImageSize {
  width: number;
  height: number;
}

const EXACT_IMAGE_SIZE_PATTERN = /^(\d{2,5})\s*[*x]\s*(\d{2,5})$/i;
const LEGACY_IMAGE_SIZE_WIDTH_MAP: Record<string, number> = {
  '0.5K': 512,
  '1K': 1024,
  '2K': 2048,
  '4K': 4096,
};

function greatestCommonDivisor(left: number, right: number): number {
  let a = Math.abs(Math.round(left));
  let b = Math.abs(Math.round(right));

  while (b !== 0) {
    const remainder = a % b;
    a = b;
    b = remainder;
  }

  return a || 1;
}

export function parseExactImageSize(value: string | null | undefined): ParsedExactImageSize | null {
  const trimmed = value?.trim() ?? '';
  if (!trimmed) {
    return null;
  }

  const matched = trimmed.match(EXACT_IMAGE_SIZE_PATTERN);
  if (!matched) {
    return null;
  }

  const width = Number(matched[1]);
  const height = Number(matched[2]);
  if (!Number.isFinite(width) || width <= 0 || !Number.isFinite(height) || height <= 0) {
    return null;
  }

  return { width, height };
}

export function normalizeImageSizeValue(value: string | null | undefined): string {
  const parsed = parseExactImageSize(value);
  if (parsed) {
    return `${parsed.width}*${parsed.height}`;
  }

  return value?.trim() ?? '';
}

export function deriveAspectRatioFromSize(value: string | null | undefined): string | null {
  const parsed = parseExactImageSize(value);
  if (!parsed) {
    return null;
  }

  const divisor = greatestCommonDivisor(parsed.width, parsed.height);
  return `${Math.round(parsed.width / divisor)}:${Math.round(parsed.height / divisor)}`;
}

export function resolveGenerationSizeDimensions(
  size: string | null | undefined,
  aspectRatio: string = DEFAULT_ASPECT_RATIO
): ParsedExactImageSize {
  const parsed = parseExactImageSize(size);
  if (parsed) {
    return parsed;
  }

  const normalized = size?.trim().toUpperCase() ?? '';
  const width = LEGACY_IMAGE_SIZE_WIDTH_MAP[normalized];
  if (typeof width === 'number') {
    const safeAspectRatio = Math.max(0.1, parseAspectRatio(aspectRatio));
    return {
      width,
      height: Math.max(1, Math.round(width / safeAspectRatio)),
    };
  }

  return { width: 1024, height: 1024 };
}
