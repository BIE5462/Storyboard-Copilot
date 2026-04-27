import { Suspense, lazy, memo } from 'react';
import { type NodeProps } from '@xyflow/react';
import { FileText } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { CANVAS_NODE_TYPES, type TextAnnotationNodeData } from '@/features/canvas/domain/canvasNodes';
import { resolveNodeDisplayName } from '@/features/canvas/domain/nodeDisplay';
import { NodeHeader, NODE_HEADER_FLOATING_POSITION_CLASS } from '@/features/canvas/ui/NodeHeader';
import { NodeResizeHandle } from '@/features/canvas/ui/NodeResizeHandle';
import { useCanvasStore } from '@/stores/canvasStore';

const TextAnnotationMarkdownPreview = lazy(() =>
  import('./TextAnnotationMarkdownPreview').then((module) => ({
    default: module.TextAnnotationMarkdownPreview,
  }))
);

type TextAnnotationNodeProps = NodeProps & {
  id: string;
  data: TextAnnotationNodeData;
  selected?: boolean;
};

const DEFAULT_WIDTH = 300;
const DEFAULT_HEIGHT = 180;
const MIN_WIDTH = 180;
const MIN_HEIGHT = 100;
const MAX_WIDTH = 900;
const MAX_HEIGHT = 900;

export const TextAnnotationNode = memo(({
  id,
  data,
  selected,
  width,
  height,
}: TextAnnotationNodeProps) => {
  const { t } = useTranslation();
  const setSelectedNode = useCanvasStore((state) => state.setSelectedNode);
  const updateNodeData = useCanvasStore((state) => state.updateNodeData);
  const content = typeof data.content === 'string' ? data.content : '';
  const resolvedTitle = resolveNodeDisplayName(CANVAS_NODE_TYPES.textAnnotation, data, t);
  const resolvedWidth = Math.max(MIN_WIDTH, Math.round(width ?? DEFAULT_WIDTH));
  const resolvedHeight = Math.max(MIN_HEIGHT, Math.round(height ?? DEFAULT_HEIGHT));

  return (
    <div
      className={`
        group relative h-full w-full overflow-visible rounded-[var(--node-radius)] border bg-surface-dark/85 p-1.5 transition-colors duration-150
        ${selected
          ? 'border-accent shadow-[0_0_0_1px_rgba(59,130,246,0.32)]'
          : 'border-[rgba(15,23,42,0.22)] hover:border-[rgba(15,23,42,0.34)] dark:border-[rgba(255,255,255,0.22)] dark:hover:border-[rgba(255,255,255,0.34)]'}
      `}
      style={{ width: resolvedWidth, height: resolvedHeight }}
      onClick={() => setSelectedNode(id)}
    >
      <NodeHeader
        className={NODE_HEADER_FLOATING_POSITION_CLASS}
        icon={<FileText className="h-4 w-4" />}
        titleText={resolvedTitle}
        editable
        onTitleChange={(nextTitle) => updateNodeData(id, { displayName: nextTitle })}
      />

      <NodeResizeHandle
        minWidth={MIN_WIDTH}
        minHeight={MIN_HEIGHT}
        maxWidth={MAX_WIDTH}
        maxHeight={MAX_HEIGHT}
      />

      {selected ? (
        <textarea
          autoFocus
          value={content}
          onChange={(event) => {
            const nextValue = event.target.value;
            updateNodeData(id, { content: nextValue });
          }}
          placeholder={t('node.textAnnotation.placeholder')}
          className="nodrag nowheel h-full w-full resize-none border-none bg-transparent px-1 py-0.5 text-sm leading-6 text-text-dark outline-none placeholder:text-text-muted/70"
        />
      ) : (
        <div className="nodrag nowheel h-full w-full overflow-auto px-1 py-0.5 text-sm leading-6 text-text-dark">
          {content.trim().length > 0 ? (
            <Suspense fallback={<div className="pt-1 text-text-muted">{content}</div>}>
              <TextAnnotationMarkdownPreview content={content} />
            </Suspense>
          ) : (
            <div className="pt-1 text-text-muted">{t('node.textAnnotation.empty')}</div>
          )}
        </div>
      )}
    </div>
  );
});

TextAnnotationNode.displayName = 'TextAnnotationNode';
