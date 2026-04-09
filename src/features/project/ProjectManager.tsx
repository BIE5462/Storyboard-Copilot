import { useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ChevronLeft, ChevronRight, FolderOpen, Pencil, Plus, Trash2 } from 'lucide-react';
import { useProjectStore } from '@/stores/projectStore';
import { getConfiguredApiKeyCount, useSettingsStore } from '@/stores/settingsStore';
import { UI_CONTENT_OVERLAY_INSET_CLASS } from '@/components/ui/motion';
import { UiButton, UiSelect } from '@/components/ui/primitives';
import { MissingApiKeyHint } from '@/features/settings/MissingApiKeyHint';
import { listModelProviders } from '@/features/canvas/models';
import { RenameDialog } from './RenameDialog';

type ProjectSortField = 'name' | 'createdAt' | 'updatedAt';
type SortDirection = 'asc' | 'desc';

const PROJECT_PANEL_GAP_PX = 16;
const PROJECT_PANEL_CARD_MIN_HEIGHT_PX = 172;
const PROJECT_PANEL_MAX_ROWS = 3;

function getProjectPanelColumnCount(width: number): number {
  if (width >= 1024) {
    return 3;
  }

  if (width >= 640) {
    return 2;
  }

  return 1;
}

function getProjectPanelRowCount(height: number): number {
  if (height <= 0) {
    return 1;
  }

  const estimatedRows = Math.floor(
    (height + PROJECT_PANEL_GAP_PX) / (PROJECT_PANEL_CARD_MIN_HEIGHT_PX + PROJECT_PANEL_GAP_PX)
  );

  return Math.min(PROJECT_PANEL_MAX_ROWS, Math.max(1, estimatedRows));
}

function chunkItems<T>(items: T[], chunkSize: number): T[][] {
  if (items.length === 0) {
    return [];
  }

  const size = Math.max(1, chunkSize);
  const chunks: T[][] = [];

  for (let index = 0; index < items.length; index += size) {
    chunks.push(items.slice(index, index + size));
  }

  return chunks;
}

export function ProjectManager() {
  const { t } = useTranslation();
  const [showRenameDialog, setShowRenameDialog] = useState(false);
  const [editingProjectId, setEditingProjectId] = useState<string | null>(null);
  const [editingProjectName, setEditingProjectName] = useState('');
  const [sortField, setSortField] = useState<ProjectSortField>('createdAt');
  const [sortDirection, setSortDirection] = useState<SortDirection>('desc');
  const projectPanelViewportRef = useRef<HTMLDivElement | null>(null);
  const [projectPanelSize, setProjectPanelSize] = useState(() => ({
    width: typeof window !== 'undefined' ? window.innerWidth : 1280,
    height: 420,
  }));
  const [currentProjectPage, setCurrentProjectPage] = useState(0);
  const providerIds = useMemo(() => listModelProviders().map((provider) => provider.id), []);
  const configuredApiKeyCount = useSettingsStore((state) =>
    getConfiguredApiKeyCount(state.apiKeys, providerIds)
  );

  const { projects, isOpeningProject, createProject, deleteProject, renameProject, openProject } =
    useProjectStore();

  const handleCreateProject = () => {
    setEditingProjectId(null);
    setEditingProjectName('');
    setShowRenameDialog(true);
  };

  const handleRenameClick = (id: string, name: string, e: React.MouseEvent) => {
    e.stopPropagation();
    setEditingProjectId(id);
    setEditingProjectName(name);
    setShowRenameDialog(true);
  };

  const handleDeleteClick = (id: string, e: React.MouseEvent) => {
    e.stopPropagation();
    deleteProject(id);
  };

  const handleConfirm = (name: string) => {
    if (editingProjectId) {
      renameProject(editingProjectId, name);
    } else {
      createProject(name);
    }
  };

  const formatDate = (timestamp: number) => {
    return new Date(timestamp).toLocaleDateString();
  };

  const sortedProjects = useMemo(() => {
    const list = [...projects];
    const direction = sortDirection === 'asc' ? 1 : -1;

    list.sort((a, b) => {
      if (sortField === 'name') {
        return a.name.localeCompare(b.name, 'zh-Hans-CN', { sensitivity: 'base' }) * direction;
      }

      const left = sortField === 'createdAt' ? a.createdAt : a.updatedAt;
      const right = sortField === 'createdAt' ? b.createdAt : b.updatedAt;
      return (left - right) * direction;
    });

    return list;
  }, [projects, sortDirection, sortField]);

  useEffect(() => {
    const element = projectPanelViewportRef.current;
    if (!element) {
      return;
    }

    const updateSize = () => {
      const rect = element.getBoundingClientRect();
      setProjectPanelSize({
        width: Math.max(0, Math.round(rect.width)),
        height: Math.max(0, Math.round(rect.height)),
      });
    };

    updateSize();
    const observer = new ResizeObserver(updateSize);
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  const projectPanelColumnCount = useMemo(
    () => getProjectPanelColumnCount(projectPanelSize.width),
    [projectPanelSize.width]
  );
  const projectPanelRowCount = useMemo(
    () => getProjectPanelRowCount(projectPanelSize.height),
    [projectPanelSize.height]
  );
  const projectPages = useMemo(
    () => chunkItems(sortedProjects, projectPanelColumnCount * projectPanelRowCount),
    [projectPanelColumnCount, projectPanelRowCount, sortedProjects]
  );
  const hasMultipleProjectPages = projectPages.length > 1;

  useEffect(() => {
    setCurrentProjectPage((previousPage) =>
      Math.min(previousPage, Math.max(0, projectPages.length - 1))
    );
  }, [projectPages.length]);

  const scrollToProjectPage = (pageIndex: number) => {
    const element = projectPanelViewportRef.current;
    if (!element) {
      return;
    }

    const targetPage = element.children.item(pageIndex);
    if (!(targetPage instanceof HTMLElement)) {
      return;
    }

    element.scrollTo({
      left: targetPage.offsetLeft,
      behavior: 'smooth',
    });
  };

  const handleProjectPanelScroll = () => {
    const element = projectPanelViewportRef.current;
    if (!element) {
      return;
    }

    const pageElements = Array.from(element.children) as HTMLElement[];
    if (pageElements.length === 0) {
      setCurrentProjectPage(0);
      return;
    }

    let nearestPageIndex = 0;
    let nearestDistance = Number.POSITIVE_INFINITY;

    pageElements.forEach((pageElement, index) => {
      const distance = Math.abs(pageElement.offsetLeft - element.scrollLeft);
      if (distance < nearestDistance) {
        nearestDistance = distance;
        nearestPageIndex = index;
      }
    });

    setCurrentProjectPage(nearestPageIndex);
  };

  const handleProjectPanelWheel = (event: React.WheelEvent<HTMLDivElement>) => {
    if (Math.abs(event.deltaX) > Math.abs(event.deltaY)) {
      return;
    }

    const element = event.currentTarget;
    if (element.scrollWidth <= element.clientWidth) {
      return;
    }

    event.preventDefault();
    element.scrollBy({
      left: event.deltaY,
      behavior: 'auto',
    });
  };

  return (
    <div className="h-full w-full overflow-hidden p-6 md:p-8">
      <div className="mx-auto flex h-full max-w-5xl flex-col">
        <div className="mb-8 flex flex-col gap-4 lg:flex-row lg:items-center lg:justify-between">
          <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:gap-4">
            <h1 className="text-2xl font-bold text-text-dark">{t('project.title')}</h1>
            <div className="flex flex-wrap items-center gap-2">
              <UiSelect
                aria-label={t('project.sortBy')}
                value={sortField}
                onChange={(event) => setSortField(event.target.value as ProjectSortField)}
                className="h-9 w-[100px] rounded-lg text-sm"
              >
                <option value="name">{t('project.sortByName')}</option>
                <option value="createdAt">{t('project.sortByCreatedAt')}</option>
                <option value="updatedAt">{t('project.sortByUpdatedAt')}</option>
              </UiSelect>
              <UiSelect
                aria-label={t('project.sortDirection')}
                value={sortDirection}
                onChange={(event) => setSortDirection(event.target.value as SortDirection)}
                className="h-9 w-[60px] rounded-lg text-sm"
              >
                <option value="asc">{t('project.sortAsc')}</option>
                <option value="desc">{t('project.sortDesc')}</option>
              </UiSelect>
            </div>
          </div>
          <UiButton
            type="button"
            variant="primary"
            onClick={handleCreateProject}
            className="gap-2 self-start lg:self-auto"
          >
            <Plus className="w-5 h-5" />
            {t('project.newProject')}
          </UiButton>
        </div>

        {configuredApiKeyCount === 0 && <MissingApiKeyHint className="mb-6" />}

        {projects.length === 0 ? (
          <div className="flex flex-1 flex-col items-center justify-center py-20 text-text-muted">
            <FolderOpen className="w-16 h-16 mb-4 opacity-50" />
            <p className="text-lg">{t('project.empty')}</p>
            <p className="text-sm mt-2">{t('project.emptyHint')}</p>
          </div>
        ) : (
          <div className="flex min-h-0 flex-1 flex-col">
            {hasMultipleProjectPages && (
              <div className="mb-3 flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                <p className="text-xs text-text-muted">{t('project.previewHint')}</p>
                <div className="flex items-center gap-2 self-end sm:self-auto">
                  <span className="min-w-[96px] text-center text-xs font-medium text-text-muted">
                    {t('project.pageIndicator', {
                      current: currentProjectPage + 1,
                      total: projectPages.length,
                    })}
                  </span>
                  <UiButton
                    type="button"
                    size="sm"
                    variant="muted"
                    className="h-9 w-9 rounded-full px-0"
                    aria-label={t('project.previousPage')}
                    disabled={currentProjectPage === 0}
                    onClick={() => scrollToProjectPage(currentProjectPage - 1)}
                  >
                    <ChevronLeft className="h-4 w-4" />
                  </UiButton>
                  <UiButton
                    type="button"
                    size="sm"
                    variant="muted"
                    className="h-9 w-9 rounded-full px-0"
                    aria-label={t('project.nextPage')}
                    disabled={currentProjectPage >= projectPages.length - 1}
                    onClick={() => scrollToProjectPage(currentProjectPage + 1)}
                  >
                    <ChevronRight className="h-4 w-4" />
                  </UiButton>
                </div>
              </div>
            )}

            <div className="relative min-h-0 flex-1">
              {hasMultipleProjectPages && currentProjectPage > 0 && (
                <div className="pointer-events-none absolute inset-y-0 left-0 z-10 w-12 bg-gradient-to-r from-bg-dark via-bg-dark/80 to-transparent" />
              )}
              {hasMultipleProjectPages && currentProjectPage < projectPages.length - 1 && (
                <div className="pointer-events-none absolute inset-y-0 right-0 z-10 w-12 bg-gradient-to-l from-bg-dark via-bg-dark/80 to-transparent" />
              )}

              <div
                ref={projectPanelViewportRef}
                className="ui-scrollbar flex h-full snap-x snap-mandatory gap-4 overflow-x-auto overflow-y-hidden pb-3"
                onScroll={handleProjectPanelScroll}
                onWheel={handleProjectPanelWheel}
              >
                {projectPages.map((page, pageIndex) => (
                  <div
                    key={`${pageIndex}-${page.length}`}
                    className="grid h-full min-w-full snap-start gap-4"
                    style={{
                      gridTemplateColumns: `repeat(${projectPanelColumnCount}, minmax(0, 1fr))`,
                      gridTemplateRows: `repeat(${projectPanelRowCount}, minmax(0, 1fr))`,
                    }}
                  >
                    {page.map((project) => (
                      <div
                        key={project.id}
                        onClick={() => openProject(project.id)}
                        className="group flex h-full min-h-[156px] cursor-pointer flex-col rounded-lg border border-border-dark bg-surface-dark p-4 transition-all hover:border-primary/50 hover:shadow-lg"
                      >
                        <div className="mb-2 flex items-start justify-between gap-2">
                          <h3 className="flex-1 truncate font-semibold text-text-dark">
                            {project.name}
                          </h3>
                          <div className="flex items-center gap-1 opacity-0 transition-opacity group-hover:opacity-100">
                            <button
                              type="button"
                              onClick={(e) => handleRenameClick(project.id, project.name, e)}
                              className="rounded p-1 hover:bg-bg-dark"
                              title={t('project.rename')}
                            >
                              <Pencil className="h-4 w-4 text-text-muted hover:text-text-dark" />
                            </button>
                            <button
                              type="button"
                              onClick={(e) => handleDeleteClick(project.id, e)}
                              className="rounded p-1 hover:bg-bg-dark"
                              title={t('project.delete')}
                            >
                              <Trash2 className="h-4 w-4 text-text-muted hover:text-red-500" />
                            </button>
                          </div>
                        </div>
                        <div className="mt-auto text-xs text-text-muted">
                          <p>
                            {t('project.modified')}: {formatDate(project.updatedAt)}
                          </p>
                          <p>
                            {t('project.created')}: {formatDate(project.createdAt)}
                          </p>
                        </div>
                      </div>
                    ))}
                  </div>
                ))}
              </div>
            </div>
          </div>
        )}
      </div>

      {isOpeningProject && (
        <div className={`pointer-events-none fixed ${UI_CONTENT_OVERLAY_INSET_CLASS} bg-black/10`} />
      )}

      <RenameDialog
        isOpen={showRenameDialog}
        title={editingProjectId ? t('project.renameTitle') : t('project.newProjectTitle')}
        defaultValue={editingProjectName}
        onClose={() => setShowRenameDialog(false)}
        onConfirm={handleConfirm}
      />
    </div>
  );
}
