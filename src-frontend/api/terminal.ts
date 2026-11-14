import { listen } from "@tauri-apps/api/event";
import { command } from "./tauri";

export interface TerminalOpenResult {
  sessionId: string;
  reused: boolean;
}
export interface TerminalOutputEvent {
  sessionId: string;
  seq: number;
  data: string;
}
export interface TerminalStatusEvent {
  sessionId: string;
  seq: number;
  status: "open" | "closed" | "error";
  message?: string;
}

export function decodeTerminalOutput(value: string) {
  const binary = atob(value);
  return Uint8Array.from(binary, (character) => character.charCodeAt(0));
}

export const terminalApi = {
  async open(nodeId: string, containerId: string, cols: number, rows: number, sessionId: string) {
    const result = await command<TerminalOpenResult>("terminal_open", {
      nodeId,
      containerId,
      sessionId,
      cols,
      rows,
    });
    return result.sessionId;
  },
  input(sessionId: string, data: string) {
    return command<boolean>("terminal_input", { sessionId, data });
  },
  resize(sessionId: string, cols: number, rows: number) {
    return command<boolean>("terminal_resize", { sessionId, cols, rows });
  },
  close(sessionId: string) {
    return command<boolean>("terminal_close", { sessionId });
  },
  onOutput(handler: (event: TerminalOutputEvent) => void) {
    return listen<TerminalOutputEvent>("terminal-output", (event) => handler(event.payload));
  },
  onStatus(handler: (event: TerminalStatusEvent) => void) {
    return listen<TerminalStatusEvent>("terminal-status", (event) => handler(event.payload));
  },
};
