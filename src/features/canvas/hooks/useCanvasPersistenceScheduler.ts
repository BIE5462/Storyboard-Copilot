import { useCallback, useRef, type MutableRefObject } from 'react';
import type { Viewport } from '@xyflow/react';

import { useCanvasStore } from '@/stores/canvasStore';
import { useProjectStore } from '@/stores/projectStore';

interface UseCanvasPersistenceSchedulerParams {
  isRestoringCanvasRef: MutableRefObject<boolean>;
  reactFlowInstance: {
    getViewport: () => Viewport;
  };
}

export function useCanvasPersistenceScheduler({
  isRestoringCanvasRef,
  reactFlowInstance,
}: UseCanvasPersistenceSchedulerParams) {
  const saveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const getCurrentProject = useProjectStore((state) => state.getCurrentProject);
  const saveCurrentProject = useProjectStore((state) => state.saveCurrentProject);

  const persistCanvasSnapshot = useCallback(() => {
    if (isRestoringCanvasRef.current) {
      return;
    }

    const currentProject = getCurrentProject();
    if (!currentProject) {
      return;
    }

    const currentNodes = useCanvasStore.getState().nodes;
    const currentEdges = useCanvasStore.getState().edges;
    const currentHistory = useCanvasStore.getState().history;
    saveCurrentProject(currentNodes, currentEdges, reactFlowInstance.getViewport(), currentHistory);
  }, [getCurrentProject, isRestoringCanvasRef, reactFlowInstance, saveCurrentProject]);

  const clearScheduledCanvasPersist = useCallback(() => {
    if (!saveTimerRef.current) {
      return;
    }

    clearTimeout(saveTimerRef.current);
    saveTimerRef.current = null;
  }, []);

  const scheduleCanvasPersist = useCallback(
    (delayMs = 140) => {
      clearScheduledCanvasPersist();
      saveTimerRef.current = setTimeout(() => {
        saveTimerRef.current = null;
        persistCanvasSnapshot();
      }, delayMs);
    },
    [clearScheduledCanvasPersist, persistCanvasSnapshot]
  );

  return {
    clearScheduledCanvasPersist,
    persistCanvasSnapshot,
    scheduleCanvasPersist,
  };
}
