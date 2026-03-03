import {
  type KeyboardEvent,
  memo,
  useMemo,
  useState,
  useCallback,
  useEffect,
  useRef,
} from 'react';
import { NodeToolbar as ReactFlowNodeToolbar, Position } from '@xyflow/react';
import {
  ArrowUp,
} from 'lucide-react';

import {
  AUTO_REQUEST_ASPECT_RATIO,
  isImageEditNode,
  type ImageSize,
  type CanvasNode,
} from '@/features/canvas/domain/canvasNodes';
import {
  canvasAiGateway,
  graphImageResolver,
} from '@/features/canvas/application/canvasServices';
import {
  detectAspectRatio,
  prepareNodeImage,
  parseAspectRatio,
} from '@/features/canvas/application/imageData';
import {
  DEFAULT_IMAGE_MODEL_ID,
  getImageModel,
  listImageModels,
} from '@/features/canvas/models';
import { useCanvasStore } from '@/stores/canvasStore';
import { useSettingsStore } from '@/stores/settingsStore';
import { ModelParamsControls } from './ModelParamsControls';
import {
  UiButton,
  UiPanel,
  UiTextAreaField,
} from '@/components/ui';

interface NodePromptInputProps {
  node: CanvasNode;
}

interface AspectRatioChoice {
  value: string;
  label: string;
}

const AUTO_ASPECT_RATIO_OPTION: AspectRatioChoice = {
  value: AUTO_REQUEST_ASPECT_RATIO,
  label: '自动',
};

function pickClosestAspectRatio(
  targetRatio: number,
  supportedAspectRatios: string[]
): string {
  const supported = supportedAspectRatios.length > 0 ? supportedAspectRatios : ['1:1'];
  let bestValue = supported[0];
  let bestDistance = Number.POSITIVE_INFINITY;

  for (const aspectRatio of supported) {
    const ratio = parseAspectRatio(aspectRatio);
    const distance = Math.abs(Math.log(ratio / targetRatio));
    if (distance < bestDistance) {
      bestDistance = distance;
      bestValue = aspectRatio;
    }
  }

  return bestValue;
}

export const NodePromptInput = memo(({ node }: NodePromptInputProps) => {
  const [error, setError] = useState<string | null>(null);

  const containerRef = useRef<HTMLDivElement>(null);
  const promptRef = useRef<HTMLTextAreaElement>(null);
  const [showImagePicker, setShowImagePicker] = useState(false);
  const [pickerCursor, setPickerCursor] = useState<number | null>(null);

  const nodes = useCanvasStore((state) => state.nodes);
  const edges = useCanvasStore((state) => state.edges);
  const updateNodeData = useCanvasStore((state) => state.updateNodeData);
  const setSelectedNode = useCanvasStore((state) => state.setSelectedNode);
  const apiKey = useSettingsStore((state) => state.apiKey);

  const imageEditNode = isImageEditNode(node) ? node : null;

  const incomingImages = useMemo(
    () => graphImageResolver.collectInputImages(imageEditNode?.id ?? '', nodes, edges),
    [imageEditNode?.id, nodes, edges]
  );

  const imageModels = useMemo(() => listImageModels(), []);

  const selectedModel = useMemo(() => {
    const modelId = imageEditNode?.data.model ?? DEFAULT_IMAGE_MODEL_ID;
    return getImageModel(modelId);
  }, [imageEditNode?.data.model]);

  const selectedResolution = useMemo(
    () =>
      selectedModel.resolutions.find((item) => item.value === imageEditNode?.data.size) ??
      selectedModel.resolutions.find((item) => item.value === selectedModel.defaultResolution) ??
      selectedModel.resolutions[0],
    [imageEditNode?.data.size, selectedModel]
  );

  const aspectRatioOptions = useMemo<AspectRatioChoice[]>(
    () => [AUTO_ASPECT_RATIO_OPTION, ...selectedModel.aspectRatios],
    [selectedModel.aspectRatios]
  );

  const selectedAspectRatio = useMemo(
    () =>
      aspectRatioOptions.find((item) => item.value === imageEditNode?.data.requestAspectRatio) ??
      AUTO_ASPECT_RATIO_OPTION,
    [aspectRatioOptions, imageEditNode?.data.requestAspectRatio]
  );

  const requestResolution = selectedModel.resolveRequest({
    referenceImageCount: incomingImages.length,
  });

  const supportedAspectRatioValues = useMemo(
    () => selectedModel.aspectRatios.map((item) => item.value),
    [selectedModel.aspectRatios]
  );

  useEffect(() => {
    if (!imageEditNode) {
      return;
    }

    if (imageEditNode.data.model !== selectedModel.id) {
      updateNodeData(imageEditNode.id, { model: selectedModel.id });
    }

    if (imageEditNode.data.size !== selectedResolution.value) {
      updateNodeData(imageEditNode.id, { size: selectedResolution.value as ImageSize });
    }

    if (imageEditNode.data.requestAspectRatio !== selectedAspectRatio.value) {
      updateNodeData(imageEditNode.id, { requestAspectRatio: selectedAspectRatio.value });
    }
  }, [
    imageEditNode,
    selectedModel.id,
    selectedResolution.value,
    selectedAspectRatio.value,
    updateNodeData,
  ]);

  useEffect(() => {
    if (incomingImages.length === 0) {
      setShowImagePicker(false);
      setPickerCursor(null);
    }
  }, [incomingImages.length]);

  useEffect(() => {
    const handleOutside = (event: MouseEvent) => {
      if (containerRef.current?.contains(event.target as globalThis.Node)) {
        return;
      }

      setShowImagePicker(false);
    };

    document.addEventListener('mousedown', handleOutside, true);
    return () => {
      document.removeEventListener('mousedown', handleOutside, true);
    };
  }, []);

  const handleGenerate = useCallback(async () => {
    if (!imageEditNode) {
      return;
    }
    if (imageEditNode.data.isGenerating) {
      return;
    }

    const prompt = imageEditNode.data.prompt.replace(/@(?=图\d+)/g, '').trim();
    if (!prompt) {
      setError('请输入提示词');
      return;
    }

    if (!apiKey) {
      setError('请在设置中填写 API Key');
      return;
    }

    const generationDurationMs = selectedModel.expectedDurationMs ?? 60000;
    updateNodeData(imageEditNode.id, {
      isGenerating: true,
      generationStartedAt: Date.now(),
      generationDurationMs,
    });
    setSelectedNode(null);
    setError(null);

    try {
      await canvasAiGateway.setApiKey('ppio', apiKey);

      let resolvedRequestAspectRatio = selectedAspectRatio.value;
      if (resolvedRequestAspectRatio === AUTO_REQUEST_ASPECT_RATIO) {
        if (incomingImages.length > 0) {
          try {
            const sourceAspectRatio = await detectAspectRatio(incomingImages[0]);
            const sourceAspectRatioValue = parseAspectRatio(sourceAspectRatio);
            resolvedRequestAspectRatio = pickClosestAspectRatio(
              sourceAspectRatioValue,
              supportedAspectRatioValues
            );
          } catch {
            resolvedRequestAspectRatio = pickClosestAspectRatio(1, supportedAspectRatioValues);
          }
        } else {
          resolvedRequestAspectRatio = pickClosestAspectRatio(1, supportedAspectRatioValues);
        }
      }

      const resultUrl = await canvasAiGateway.generateImage({
        prompt,
        model: requestResolution.requestModel,
        size: selectedResolution.value,
        aspectRatio: resolvedRequestAspectRatio,
        referenceImages: incomingImages,
      });

      const prepared = await prepareNodeImage(resultUrl);

      updateNodeData(imageEditNode.id, {
        imageUrl: prepared.imageUrl,
        previewImageUrl: prepared.previewImageUrl,
        aspectRatio: prepared.aspectRatio,
        requestAspectRatio: selectedAspectRatio.value,
        isGenerating: false,
        generationStartedAt: null,
      });
    } catch (generationError) {
      setError(generationError instanceof Error ? generationError.message : '生成失败');
      updateNodeData(imageEditNode.id, {
        isGenerating: false,
        generationStartedAt: null,
      });
    }
  }, [
    apiKey,
    imageEditNode,
    incomingImages,
    requestResolution.requestModel,
    selectedModel.expectedDurationMs,
    supportedAspectRatioValues,
    setSelectedNode,
    selectedAspectRatio.value,
    selectedResolution.value,
    updateNodeData,
  ]);

  if (!imageEditNode) {
    return null;
  }

  const handlePromptKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key === '@' && incomingImages.length > 0) {
      event.preventDefault();
      const cursor = event.currentTarget.selectionStart ?? imageEditNode.data.prompt.length;
      setPickerCursor(cursor);
      setShowImagePicker(true);
      return;
    }

    if (event.key === 'Escape' && showImagePicker) {
      event.preventDefault();
      setShowImagePicker(false);
      setPickerCursor(null);
    }
  };

  const insertImageReference = (imageIndex: number) => {
    const marker = `图${imageIndex + 1}`;
    const currentPrompt = imageEditNode.data.prompt;
    const cursor = pickerCursor ?? currentPrompt.length;
    const nextPrompt = `${currentPrompt.slice(0, cursor)}${marker}${currentPrompt.slice(cursor)}`;

    updateNodeData(imageEditNode.id, { prompt: nextPrompt });
    setShowImagePicker(false);

    const nextCursor = cursor + marker.length;
    requestAnimationFrame(() => {
      promptRef.current?.focus();
      promptRef.current?.setSelectionRange(nextCursor, nextCursor);
    });
  };

  return (
    <ReactFlowNodeToolbar
      nodeId={imageEditNode.id}
      isVisible
      position={Position.Bottom}
      align="center"
      offset={14}
      className="pointer-events-auto"
    >
      <div ref={containerRef} className="relative">
        <UiPanel className="w-[540px] p-2">
          <UiTextAreaField
            ref={promptRef}
            value={imageEditNode.data.prompt}
            onChange={(event) => updateNodeData(imageEditNode.id, { prompt: event.target.value })}
            onKeyDown={handlePromptKeyDown}
            placeholder="描述任何你想要生成或编辑的内容"
            className="h-32 border-none bg-transparent px-1 py-1.5 text-sm leading-7 placeholder:text-text-muted/80 focus:border-transparent"
          />

          {showImagePicker && incomingImages.length > 0 && (
            <div className="absolute left-2 top-2 z-30 w-[220px] rounded-xl border border-[rgba(255,255,255,0.16)] bg-surface-dark p-2 shadow-xl">
              <div className="mb-2 text-lg leading-none text-text-dark">@</div>
              <div className="max-h-[180px] space-y-1 overflow-y-auto">
                {incomingImages.map((imageUrl, index) => (
                  <button
                    key={`${imageUrl}-${index}`}
                    type="button"
                    onClick={() => insertImageReference(index)}
                    className="flex w-full items-center gap-2 rounded-lg border border-transparent bg-bg-dark/70 px-2 py-2 text-left text-sm text-text-dark transition-colors hover:border-[rgba(255,255,255,0.18)]"
                  >
                    <img
                      src={imageUrl}
                      alt={`图${index + 1}`}
                      className="h-8 w-8 rounded object-cover"
                    />
                    <span>{`图${index + 1}`}</span>
                  </button>
                ))}
              </div>
            </div>
          )}

          <div className="mt-1 flex items-center gap-1">
            <ModelParamsControls
              imageModels={imageModels}
              selectedModel={selectedModel}
              selectedResolution={selectedResolution}
              selectedAspectRatio={selectedAspectRatio}
              aspectRatioOptions={aspectRatioOptions}
              onModelChange={(modelId) => updateNodeData(imageEditNode.id, { model: modelId })}
              onResolutionChange={(resolution) =>
                updateNodeData(imageEditNode.id, { size: resolution as ImageSize })
              }
              onAspectRatioChange={(aspectRatio) =>
                updateNodeData(imageEditNode.id, { requestAspectRatio: aspectRatio })
              }
            />

            <div className="ml-auto" />

            <UiButton
              onClick={handleGenerate}
              variant="primary"
              className="h-10 w-10 rounded-full px-0"
            >
              <ArrowUp className="h-5 w-5" strokeWidth={2.8} />
            </UiButton>
          </div>

          {error && <div className="mt-2 text-xs text-red-400">{error}</div>}
        </UiPanel>

      </div>
    </ReactFlowNodeToolbar>
  );
});

NodePromptInput.displayName = 'NodePromptInput';
