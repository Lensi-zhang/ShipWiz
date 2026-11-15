import { invoke } from "@tauri-apps/api/core";
import { alertError } from "../utils/message";
import { dict } from "../dict";
import type { ApiResponse } from "../types";

export async function command<T>(
  name: string,
  args?: Record<string, unknown>,
): Promise<T> {
  try {
    const response = await invoke<ApiResponse<T>>(name, args);
    if (response.code !== 0) {
      throw new Error(response.msg || dict.errors.unknown);
    }
    if (response.data === null) {
      throw new Error(dict.errors.emptyData);
    }
    return response.data;
  } catch (error) {
    const text = error instanceof Error ? error.message : dict.errors.unknown;
    alertError(text, { duration: 6000, closable: true });
    throw error;
  }
}
