import { mockSnapshot } from "./mock";
import type { AppSnapshot, ServiceAction, ServiceActionResult, SettingsUpdate } from "./types";

function withFallback(message: string): AppSnapshot {
  return {
    ...mockSnapshot,
    warnings: [message, ...mockSnapshot.warnings],
    usingMock: true,
  };
}

function isPreviewRuntimeError(error: unknown): boolean {
  const message = String(error);

  return (
    message.includes("__TAURI_INTERNALS__") ||
    message.includes("window.__TAURI__") ||
    message.includes("Cannot read properties of undefined") ||
    message.includes("ipc") ||
    message.includes("mockIPC")
  );
}

async function invokeCommand<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(command, args);
}

export async function getAppSnapshot(): Promise<AppSnapshot> {
  try {
    return await invokeCommand<AppSnapshot>("get_app_snapshot");
  } catch (error) {
    if (!isPreviewRuntimeError(error)) {
      throw error;
    }
    return withFallback(`Falling back to sample data: ${String(error)}`);
  }
}

export async function switchAccount(accountKey: string): Promise<AppSnapshot> {
  try {
    return await invokeCommand<AppSnapshot>("switch_account", { accountKey });
  } catch (error) {
    if (!isPreviewRuntimeError(error)) {
      throw error;
    }
    return withFallback(`Switch is unavailable in browser preview for ${accountKey}.`);
  }
}

export async function updateSettings(input: SettingsUpdate): Promise<AppSnapshot> {
  try {
    return await invokeCommand<AppSnapshot>("update_settings", { input });
  } catch (error) {
    if (!isPreviewRuntimeError(error)) {
      throw error;
    }
    return withFallback("Settings updates are unavailable in browser preview.");
  }
}

export async function launchAddAccountLogin(): Promise<string> {
  try {
    return await invokeCommand<string>("launch_add_account_login");
  } catch (error) {
    if (!isPreviewRuntimeError(error)) {
      throw error;
    }
    return "Browser preview cannot launch the native Codex sign-in flow.";
  }
}

export async function manageAutoSwitchService(action: ServiceAction): Promise<ServiceActionResult> {
  try {
    return await invokeCommand<ServiceActionResult>("manage_auto_switch_service", { action });
  } catch (error) {
    if (!isPreviewRuntimeError(error)) {
      throw error;
    }
    return {
      snapshot: withFallback(`Service action "${action}" is unavailable in browser preview.`),
      message: "Browser preview cannot manage the native auto switch service.",
    };
  }
}

export async function hideCurrentWindowIfPossible(): Promise<void> {
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().hide();
  } catch (error) {
    if (!isPreviewRuntimeError(error)) {
      throw error;
    }
  }
}

export async function quitApp(): Promise<void> {
  try {
    await invokeCommand("quit_app");
  } catch (error) {
    if (!isPreviewRuntimeError(error)) {
      throw error;
    }
  }
}
