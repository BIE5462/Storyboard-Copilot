import { readFile } from 'node:fs/promises';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const REQUEST_TIMEOUT_MS = 120_000;
const MAX_ATTEMPTS = 3;
const RETRY_DELAYS_MS = [1_500, 4_000];
const DEFAULT_MODEL = 'gemini-3.1-flash-image-preview';
const DEFAULT_ASPECT_RATIO = '1:1';
const DEFAULT_IMAGE_SIZE = '2K';

function resolveMimeType(filePath) {
  const extension = path.extname(filePath).toLowerCase();
  if (extension === '.jpg' || extension === '.jpeg') return 'image/jpeg';
  if (extension === '.webp') return 'image/webp';
  if (extension === '.gif') return 'image/gif';
  return 'image/png';
}

function buildRequestBody({ prompt, imagePart }) {
  const parts = [{ text: prompt }];

  if (imagePart) {
    parts.push(imagePart);
  }

  return {
    contents: [
      {
        role: 'user',
        parts,
      },
    ],
    generationConfig: {
      responseModalities: ['IMAGE', 'TEXT'],
      imageConfig: {
        aspectRatio: DEFAULT_ASPECT_RATIO,
        imageSize: DEFAULT_IMAGE_SIZE,
      },
    },
  };
}

function extractInlineImageInfo(responseJson) {
  const parts = responseJson?.candidates?.[0]?.content?.parts;
  if (!Array.isArray(parts)) {
    return null;
  }

  const inlinePart = parts.find((part) => part?.inlineData?.data);
  if (!inlinePart?.inlineData?.data) {
    return null;
  }

  return {
    mimeType: inlinePart.inlineData.mimeType ?? 'unknown',
    base64Length: inlinePart.inlineData.data.length,
  };
}

async function callQianhaiApi({ apiKey, endpoint, name, body }) {
  for (let attempt = 1; attempt <= MAX_ATTEMPTS; attempt += 1) {
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(new Error('request timeout')), REQUEST_TIMEOUT_MS);
    const startedAt = Date.now();

    try {
      const response = await fetch(endpoint, {
        method: 'POST',
        headers: {
          Authorization: `Bearer ${apiKey}`,
          'Content-Type': 'application/json',
        },
        body: JSON.stringify(body),
        signal: controller.signal,
      });

      const rawText = await response.text();
      const elapsedMs = Date.now() - startedAt;

      let responseJson = null;
      try {
        responseJson = JSON.parse(rawText);
      } catch {
        responseJson = null;
      }

      const inlineImage = extractInlineImageInfo(responseJson);
      const success = response.ok && inlineImage !== null;
      const shouldRetry =
        attempt < MAX_ATTEMPTS
        && (!response.ok || inlineImage === null)
        && (response.status === 429 || response.status >= 500);

      console.log(`\n[${name}]`);
      console.log(`attempt=${attempt}/${MAX_ATTEMPTS}`);
      console.log(`status=${response.status} ${response.statusText}`);
      console.log(`elapsed_ms=${elapsedMs}`);
      console.log(`has_inline_image=${inlineImage ? 'yes' : 'no'}`);
      if (inlineImage) {
        console.log(`inline_image_mime=${inlineImage.mimeType}`);
        console.log(`inline_image_base64_length=${inlineImage.base64Length}`);
      }
      if (!success) {
        console.log(`response_preview=${rawText.slice(0, 1000)}`);
      }
      if (shouldRetry) {
        const retryDelay = RETRY_DELAYS_MS[attempt - 1] ?? RETRY_DELAYS_MS[RETRY_DELAYS_MS.length - 1];
        console.log(`retry_in_ms=${retryDelay}`);
        await new Promise((resolve) => setTimeout(resolve, retryDelay));
        continue;
      }

      return success;
    } catch (error) {
      const elapsedMs = Date.now() - startedAt;
      const shouldRetry = attempt < MAX_ATTEMPTS;

      console.log(`\n[${name}]`);
      console.log(`attempt=${attempt}/${MAX_ATTEMPTS}`);
      console.log(`elapsed_ms=${elapsedMs}`);
      console.log(`network_error=${error?.message ?? String(error)}`);
      if (shouldRetry) {
        const retryDelay = RETRY_DELAYS_MS[attempt - 1] ?? RETRY_DELAYS_MS[RETRY_DELAYS_MS.length - 1];
        console.log(`retry_in_ms=${retryDelay}`);
        await new Promise((resolve) => setTimeout(resolve, retryDelay));
        continue;
      }

      return false;
    } finally {
      clearTimeout(timeout);
    }
  }

  return false;
}

async function main() {
  const apiKey = process.env.QIANHAI_API_KEY?.trim();
  if (!apiKey) {
    console.error('Missing QIANHAI_API_KEY environment variable.');
    process.exitCode = 1;
    return;
  }

  const scriptDir = path.dirname(fileURLToPath(import.meta.url));
  const repoRoot = path.resolve(scriptDir, '..');
  const referenceImagePath = process.argv[2]
    ? path.resolve(process.argv[2])
    : path.join(repoRoot, 'src-tauri', 'icons', '32x32.png');
  const endpoint = `https://api.qianhai.online/v1beta/models/${DEFAULT_MODEL}:generateContent`;

  const referenceBytes = await readFile(referenceImagePath);
  const referenceImagePart = {
    inlineData: {
      mimeType: resolveMimeType(referenceImagePath),
      data: referenceBytes.toString('base64'),
    },
  };

  console.log(`model=${DEFAULT_MODEL}`);
  console.log(`endpoint=${endpoint}`);
  console.log(`reference_image_path=${referenceImagePath}`);
  console.log(`reference_image_bytes=${referenceBytes.byteLength}`);
  console.log(`timeout_ms=${REQUEST_TIMEOUT_MS}`);

  const noReferenceSuccess = await callQianhaiApi({
    apiKey,
    endpoint,
    name: 'no-reference-image',
    body: buildRequestBody({
      prompt: '生成一张1:1方图，一朵小红花，白色背景，禁止添加文字。',
    }),
  });

  const withReferenceSuccess = await callQianhaiApi({
    apiKey,
    endpoint,
    name: 'with-reference-image',
    body: buildRequestBody({
      prompt: '生成一张1:1方图，参考图片的扁平简洁风格，画一朵小红花，禁止添加文字。',
      imagePart: referenceImagePart,
    }),
  });

  if (!noReferenceSuccess || !withReferenceSuccess) {
    process.exitCode = 1;
  }
}

await main();
