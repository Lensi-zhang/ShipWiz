import { command } from "./tauri";
import type {
  ContainerInfo,
  CreateContainerInput,
  ExecResult,
  FleetOverview,
} from "../types";

export const containerApi = {
  list: (nodeId: string) =>
    command<ContainerInfo[]>("list_containers", { nodeId }),
  create: (nodeId: string, input: CreateContainerInput) =>
    command<string>("create_container", { nodeId, input }),
  start: (nodeId: string, containerId: string) =>
    command<boolean>("start_container", { nodeId, containerId }),
  stop: (nodeId: string, containerId: string) =>
    command<boolean>("stop_container", { nodeId, containerId }),
  restart: (nodeId: string, containerId: string) =>
    command<boolean>("restart_container", { nodeId, containerId }),
  remove: (nodeId: string, containerId: string) =>
    command<boolean>("remove_container", { nodeId, containerId }),
  logs: (nodeId: string, containerId: string, tail: number) =>
    command<string>("get_container_logs", { nodeId, containerId, tail }),
  exec: (nodeId: string, containerId: string, execCommand: string) =>
    command<ExecResult>("exec_container", {
      nodeId,
      containerId,
      command: execCommand,
    }),
  overview: () => command<FleetOverview>("get_fleet_overview"),
};
