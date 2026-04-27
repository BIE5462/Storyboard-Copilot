export interface PickDirectoryOptions {
  title?: string;
}

export function isTauriRuntimeSync(): boolean {
  return typeof window !== 'undefined' && Boolean('__TAURI_INTERNALS__' in window);
}

export function convertFileSrcPath(path: string): string {
  if (!isTauriRuntimeSync()) {
    return path;
  }

  const tauriGlobal = window as typeof window & {
    __TAURI_INTERNALS__?: {
      convertFileSrc?: (filePath: string) => string;
    };
  };
  return tauriGlobal.__TAURI_INTERNALS__?.convertFileSrc?.(path) ?? path;
}

export async function notifyFrontendReady(): Promise<void> {
  const { invoke } = await import('@tauri-apps/api/core');
  await invoke('frontend_ready');
}

export async function isTauriRuntime(): Promise<boolean> {
  try {
    if (!isTauriRuntimeSync()) {
      return false;
    }
    const { isTauri } = await import('@tauri-apps/api/core');
    return isTauri();
  } catch {
    return false;
  }
}

export async function getAppVersion(): Promise<string> {
  try {
    const { getVersion } = await import('@tauri-apps/api/app');
    return await getVersion();
  } catch {
    return '';
  }
}

export async function pickDirectory(options: PickDirectoryOptions = {}): Promise<string | null> {
  const { open } = await import('@tauri-apps/plugin-dialog');
  const selected = await open({
    directory: true,
    multiple: false,
    title: options.title,
  });

  if (!selected || Array.isArray(selected)) {
    return null;
  }

  return selected;
}

export async function openExternalUrl(url: string): Promise<void> {
  try {
    const { openUrl } = await import('@tauri-apps/plugin-opener');
    await openUrl(url);
  } catch {
    window.open(url, '_blank', 'noopener,noreferrer');
  }
}

export async function openPathInSystem(path: string): Promise<void> {
  const { openPath } = await import('@tauri-apps/plugin-opener');
  await openPath(path);
}

export async function revealPathInDirectory(path: string): Promise<void> {
  const { revealItemInDir } = await import('@tauri-apps/plugin-opener');
  await revealItemInDir(path);
}

export async function joinPath(...paths: string[]): Promise<string> {
  const { join } = await import('@tauri-apps/api/path');
  return await join(...paths);
}

export async function minimizeWindow(): Promise<void> {
  const { getCurrentWindow } = await import('@tauri-apps/api/window');
  await getCurrentWindow().minimize();
}

export async function toggleMaximizeWindow(): Promise<void> {
  const { getCurrentWindow } = await import('@tauri-apps/api/window');
  const appWindow = getCurrentWindow();
  const isMaximized = await appWindow.isMaximized();
  if (isMaximized) {
    await appWindow.unmaximize();
  } else {
    await appWindow.maximize();
  }
}

export async function closeWindow(): Promise<void> {
  const { getCurrentWindow } = await import('@tauri-apps/api/window');
  await getCurrentWindow().close();
}

export async function startWindowDrag(): Promise<void> {
  const { getCurrentWindow } = await import('@tauri-apps/api/window');
  await getCurrentWindow().startDragging();
}
