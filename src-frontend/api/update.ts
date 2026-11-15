import { invoke } from "@tauri-apps/api/core";
import type { ApiResponse, UpdateInfo } from "../types";

async function updateCommand<T>(
  name: string,
  args?: Record<string, unknown>,
): Promise<T> {
  const response = await invoke<ApiResponse<T>>(name, args);
  if (response.code !== 0) throw new Error(response.msg);
  return response.data as T;
}

/** 自动检查可使用 24 小时缓存；手动检查传入 true 强制联网并绕过跳过版本。 */
export function checkUpdate(force = false): Promise<UpdateInfo | null> {
  return updateCommand<UpdateInfo | null>("check_update", { force });
}

export function skipUpdate(version: string): Promise<null> {
  return updateCommand<null>("skip_update", { version });
}

export function clearSkippedUpdate(): Promise<null> {
  return updateCommand<null>("clear_skipped_update");
}

export function getSkippedUpdate(): Promise<string | null> {
  return updateCommand<string | null>("get_skipped_update");
}
