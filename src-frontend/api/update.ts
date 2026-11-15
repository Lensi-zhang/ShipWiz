import { invoke } from "@tauri-apps/api/core";
import type { ApiResponse, UpdateInfo } from "../types";

/**
 * 检查 GitHub Release 最新版本。
 * 有更新返回版本信息，已是最新返回 null；网络异常时抛出错误，由调用方决定是否提示。
 */
export async function checkUpdate(): Promise<UpdateInfo | null> {
  const response = await invoke<ApiResponse<UpdateInfo>>("check_update");
  if (response.code !== 0) {
    throw new Error(response.msg);
  }
  return response.data;
}
