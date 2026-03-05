import {
  memo,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  type ChangeEvent,
  type DragEvent,
} from 'react';
import { Handle, Position, useViewport, type NodeProps } from '@xyflow/react';
import { Upload } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import {
  CANVAS_NODE_TYPES,
  EXPORT_RESULT_NODE_MIN_HEIGHT,
  EXPORT_RESULT_NODE_MIN_WIDTH,
  type UploadImageNodeData,
} from '@/features/canvas/domain/canvasNodes';
import {
  resolveMinEdgeFittedSize,
  resolveResizeMinConstraintsByAspect,
} from '@/features/canvas/application/imageNodeSizing';
import {
  isNodeUsingDefaultDisplayName,
  resolveNodeDisplayName,
} from '@/features/canvas/domain/nodeDisplay';
import { canvasEventBus } from '@/features/canvas/application/canvasServices';
import { NodeHeader, NODE_HEADER_FLOATING_POSITION_CLASS } from '@/features/canvas/ui/NodeHeader';
import { NodeResizeHandle } from '@/features/canvas/ui/NodeResizeHandle';
import {
  isLikelyLocalImagePath,
  prepareNodeImage,
  readFileAsDataUrl,
  resolveImageDisplayUrl,
  shouldUseOriginalImageByZoom,
} from '@/features/canvas/application/imageData';
import { CanvasNodeImage } from '@/features/canvas/ui/CanvasNodeImage';
import { useCanvasStore } from '@/stores/canvasStore';
import { useSettingsStore } from '@/stores/settingsStore';

type UploadNodeProps = NodeProps & {
  id: string;
  data: UploadImageNodeData;
  selected?: boolean;
};

function resolveNodeDimension(value: number | undefined, fallback: number): number {
  if (typeof value === 'number' && Number.isFinite(value) && value > 1) {
    return Math.round(value);
  }
  return fallback;
}

export const UploadNode = memo(({ id, data, selected, width, height }: UploadNodeProps) => {
  const { t } = useTranslation();
  const setSelectedNode = useCanvasStore((state) => state.setSelectedNode);
  const updateNodeData = useCanvasStore((state) => state.updateNodeData);
  const useUploadFilenameAsNodeTitle = useSettingsStore((state) => state.useUploadFilenameAsNodeTitle);
  const { zoom } = useViewport();
  const inputRef = useRef<HTMLInputElement>(null);
  const resolvedAspectRatio = data.aspectRatio || '1:1';
  const compactSize = resolveMinEdgeFittedSize(resolvedAspectRatio, {
    minWidth: EXPORT_RESULT_NODE_MIN_WIDTH,
    minHeight: EXPORT_RESULT_NODE_MIN_HEIGHT,
  });
  const resolvedWidth = resolveNodeDimension(width, compactSize.width);
  const resolvedHeight = resolveNodeDimension(height, compactSize.height);
  const resizeConstraints = resolveResizeMinConstraintsByAspect(resolvedAspectRatio, {
    minWidth: EXPORT_RESULT_NODE_MIN_WIDTH,
    minHeight: EXPORT_RESULT_NODE_MIN_HEIGHT,
  });
  const resizeMinWidth = resizeConstraints.minWidth;
  const resizeMinHeight = resizeConstraints.minHeight;
  const resolvedTitle = useMemo(() => {
    const sourceFileName = typeof data.sourceFileName === 'string' ? data.sourceFileName.trim() : '';
    if (
      useUploadFilenameAsNodeTitle
      && sourceFileName
      && isNodeUsingDefaultDisplayName(CANVAS_NODE_TYPES.upload, data)
    ) {
      return sourceFileName;
    }

    return resolveNodeDisplayName(CANVAS_NODE_TYPES.upload, data);
  }, [data, useUploadFilenameAsNodeTitle]);

  const processFile = useCallback(
    async (file: File) => {
      const tauriFilePath =
        (file as File & { path?: string }).path;
      const normalizedPath = typeof tauriFilePath === 'string' ? tauriFilePath.trim() : '';
      const canUseLocalPath =
        normalizedPath.length > 0
        && (isLikelyLocalImagePath(normalizedPath) || normalizedPath.toLowerCase().startsWith('file://'));
      const source =
        canUseLocalPath
          ? normalizedPath
          : await readFileAsDataUrl(file);

      const prepared = await prepareNodeImage(source);
      const nextData: Partial<UploadImageNodeData> = {
        imageUrl: prepared.imageUrl,
        previewImageUrl: prepared.previewImageUrl,
        aspectRatio: prepared.aspectRatio || '1:1',
        sourceFileName: file.name,
      };
      if (useUploadFilenameAsNodeTitle) {
        nextData.displayName = file.name;
      }
      updateNodeData(id, nextData);
    },
    [id, updateNodeData, useUploadFilenameAsNodeTitle]
  );

  const handleDrop = useCallback(
    async (event: DragEvent<HTMLElement>) => {
      event.preventDefault();
      const file = event.dataTransfer.files?.[0];
      if (!file || !file.type.startsWith('image/')) {
        return;
      }

      await processFile(file);
    },
    [processFile]
  );

  const handleFileChange = useCallback(
    async (event: ChangeEvent<HTMLInputElement>) => {
      const file = event.target.files?.[0];
      if (!file || !file.type.startsWith('image/')) {
        return;
      }

      await processFile(file);
      event.target.value = '';
    },
    [processFile]
  );

  useEffect(() => {
    return canvasEventBus.subscribe('upload-node/reupload', ({ nodeId }) => {
      if (nodeId !== id) {
        return;
      }
      inputRef.current?.click();
    });
  }, [id]);

  useEffect(() => {
    return canvasEventBus.subscribe('upload-node/paste-image', ({ nodeId, file }) => {
      if (nodeId !== id || !file.type.startsWith('image/')) {
        return;
      }
      void processFile(file);
    });
  }, [id, processFile]);

  const handleNodeClick = useCallback(() => {
    setSelectedNode(id);
    if (!data.imageUrl) {
      inputRef.current?.click();
    }
  }, [data.imageUrl, id, setSelectedNode]);

  const imageSource = useMemo(() => {
    const preferOriginal = shouldUseOriginalImageByZoom(zoom);
    const picked = preferOriginal
      ? data.imageUrl || data.previewImageUrl
      : data.previewImageUrl || data.imageUrl;
    return picked ? resolveImageDisplayUrl(picked) : null;
  }, [data.imageUrl, data.previewImageUrl, zoom]);

  return (
    <div
      className={`
        group relative overflow-visible rounded-[var(--node-radius)] border bg-surface-dark/85 p-0 transition-colors duration-150
        ${selected
          ? 'border-accent shadow-[0_0_0_1px_rgba(59,130,246,0.32)]'
          : 'border-[rgba(255,255,255,0.22)] hover:border-[rgba(255,255,255,0.34)]'}
      `}
      style={{ width: resolvedWidth, height: resolvedHeight }}
      onClick={handleNodeClick}
    >
      <NodeHeader
        className={NODE_HEADER_FLOATING_POSITION_CLASS}
        icon={<Upload className="h-4 w-4" />}
        titleText={resolvedTitle}
        editable
        onTitleChange={(nextTitle) => updateNodeData(id, { displayName: nextTitle })}
      />

      {data.imageUrl ? (
        <div
          className="block h-full w-full overflow-hidden rounded-[var(--node-radius)] bg-bg-dark"
        >
          <CanvasNodeImage
            src={imageSource ?? ''}
            viewerSourceUrl={data.imageUrl ? resolveImageDisplayUrl(data.imageUrl) : null}
            alt={t('node.upload.uploadedAlt')}
            className="h-full w-full object-contain"
          />
        </div>
      ) : (
        <label
          className="block h-full w-full overflow-hidden rounded-[var(--node-radius)] bg-bg-dark"
          onDrop={handleDrop}
          onDragOver={(event) => event.preventDefault()}
        >
          <div className="flex h-full w-full cursor-pointer flex-col items-center justify-center gap-2 text-text-muted/85">
            <Upload className="h-7 w-7 opacity-60" />
            <span className="px-3 text-center text-[12px] leading-6">{t('node.upload.hint')}</span>
          </div>
        </label>
      )}
      <input
        ref={inputRef}
        type="file"
        accept="image/*"
        className="hidden"
        onChange={handleFileChange}
      />

      <Handle
        type="source"
        id="source"
        position={Position.Right}
        className="!h-2 !w-2 !border-surface-dark !bg-accent"
      />
      <NodeResizeHandle
        minWidth={resizeMinWidth}
        minHeight={resizeMinHeight}
        maxWidth={1400}
        maxHeight={1400}
      />
    </div>
  );
});

UploadNode.displayName = 'UploadNode';
