import { readFile } from 'node:fs/promises';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const REQUEST_TIMEOUT_MS = 120_000;
const DEFAULT_MODEL = 'qwen-image-2.0-pro';
const DEFAULT_SIZE = '1024*1024';
const DEFAULT_ENDPOINT =
  'https://dashscope.aliyuncs.com/api/v1/services/aigc/multimodal-generation/generation';

function resolveMimeType(filePath) {
  const extension = path.extname(filePath).toLowerCase();
  if (extension === '.jpg' || extension === '.jpeg') return 'image/jpeg';
  if (extension === '.webp') return 'image/webp';
  if (extension === '.gif') return 'image/gif';
  return 'image/png';
}

async function resolveReferenceImageSource(input) {
  if (!input) {
    return null;
  }

  const trimmed = input.trim();
  if (!trimmed) {
    return null;
  }

  if (
    trimmed.startsWith('http://')
    || trimmed.startsWith('https://')
    || trimmed.startsWith('data:image/')
  ) {
    return trimmed;
  }

  const filePath = path.resolve(trimmed);
  const bytes = await readFile(filePath);
  const mimeType = resolveMimeType(filePath);
  return `data:${mimeType};base64,${bytes.toString('base64')}`;
}

function buildRequestBody({ model, prompt, size, referenceImages }) {
  const content = [
    ...referenceImages.map((image) => ({ image })),
    { text: prompt },
  ];

  return {
    model,
    input: {
      messages: [
        {
          role: 'user',
          content,
        },
      ],
    },
    parameters: {
      n: 1,
      negative_prompt: ' ',
      prompt_extend: true,
      watermark: false,
      size,
    },
  };
}

function extractImageUrl(responseJson) {
  const content = responseJson?.output?.choices?.[0]?.message?.content;
  if (!Array.isArray(content)) {
    return null;
  }

  const imageItem = content.find((item) => typeof item?.image === 'string' && item.image.trim());
  return imageItem?.image?.trim() ?? null;
}

function extractErrorSummary(responseJson) {
  const parts = [];
  if (typeof responseJson?.code === 'string' && responseJson.code.trim()) {
    parts.push(`code=${responseJson.code.trim()}`);
  }
  if (typeof responseJson?.message === 'string' && responseJson.message.trim()) {
    parts.push(`message=${responseJson.message.trim()}`);
  }
  if (typeof responseJson?.request_id === 'string' && responseJson.request_id.trim()) {
    parts.push(`request_id=${responseJson.request_id.trim()}`);
  }
  return parts.join(', ');
}

async function callDashScopeApi({ apiKey, label, body }) {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(new Error('request timeout')), REQUEST_TIMEOUT_MS);
  const startedAt = Date.now();

  try {
    const response = await fetch(DEFAULT_ENDPOINT, {
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

    const imageUrl = extractImageUrl(responseJson);
    const errorSummary = responseJson ? extractErrorSummary(responseJson) : '';

    console.log(`\n[${label}]`);
    console.log(`status=${response.status} ${response.statusText}`);
    console.log(`elapsed_ms=${elapsedMs}`);
    console.log(`image_url=${imageUrl ?? '(none)'}`);
    if (errorSummary) {
      console.log(`error_summary=${errorSummary}`);
    }
    if (!response.ok || !imageUrl) {
      console.log(`response_preview=${rawText.slice(0, 1200)}`);
    }

    return response.ok && Boolean(imageUrl);
  } finally {
    clearTimeout(timeout);
  }
}

async function main() {
  const apiKey = process.env.DASHSCOPE_API_KEY?.trim();
  if (!apiKey) {
    console.error('Missing DASHSCOPE_API_KEY environment variable.');
    process.exitCode = 1;
    return;
  }

  const scriptDir = path.dirname(fileURLToPath(import.meta.url));
  const repoRoot = path.resolve(scriptDir, '..');
  const referenceInput = process.argv[2]
    ? path.resolve(process.argv[2])
    : path.join(repoRoot, 'src-tauri', 'icons', '32x32.png');
  const referenceImage = await resolveReferenceImageSource(referenceInput);

  console.log(`endpoint=${DEFAULT_ENDPOINT}`);
  console.log(`model=${DEFAULT_MODEL}`);
  console.log(`size=${DEFAULT_SIZE}`);
  console.log(`reference_image=${referenceInput}`);
  console.log(`timeout_ms=${REQUEST_TIMEOUT_MS}`);

  const noReferenceSuccess = await callDashScopeApi({
    apiKey,
    label: 'no-reference-image',
    body: buildRequestBody({
      model: DEFAULT_MODEL,
      prompt: '生成一张 1:1 方图，一朵放在白色背景上的红花，不要添加文字。',
      size: DEFAULT_SIZE,
      referenceImages: [],
    }),
  });

  const withReferenceSuccess = referenceImage
    ? await callDashScopeApi({
        apiKey,
        label: 'with-reference-image',
        body: buildRequestBody({
          model: DEFAULT_MODEL,
          prompt: '参考输入图的构图与色彩倾向，生成一张简洁插画风格的小红花，不要添加文字。',
          size: DEFAULT_SIZE,
          referenceImages: [referenceImage],
        }),
      })
    : true;

  if (!noReferenceSuccess || !withReferenceSuccess) {
    process.exitCode = 1;
  }
}

await main();
