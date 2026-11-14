import { command } from "./tauri";
import type {
  ComposeCommandResult,
  ComposeDeployment,
  ComposeProject,
  ComposeTemplate,
  ComposeValidation,
} from "../types";

export const composeApi = {
  projects: () => command<ComposeProject[]>("list_compose_projects"),
  saveProject: (input: { name: string; yaml: string }, id: string | null) =>
    command<ComposeProject>("save_compose_project", { input, id }),
  removeProject: (id: string) => command<boolean>("remove_compose_project", { id }),
  validate: (yaml: string) => command<ComposeValidation>("validate_compose", { yaml }),
  up: (nodeId: string, projectId: string) =>
    command<ComposeCommandResult>("compose_up", { nodeId, projectId }),
  down: (nodeId: string, projectId: string) =>
    command<ComposeCommandResult>("compose_down", { nodeId, projectId }),
  deployments: (nodeId: string) => command<ComposeDeployment[]>("compose_list", { nodeId }),
  logs: (nodeId: string, project: string, tail = 200) =>
    command<string>("compose_logs", { nodeId, project, tail }),
  templates: () => command<ComposeTemplate[]>("list_compose_templates"),
  saveTemplate: (
    input: { title: string; desc: string; tag: string[]; yaml: string },
    id: string | null,
  ) => command<ComposeTemplate[]>("save_compose_template", { input, id }),
  removeTemplate: (id: string) =>
    command<ComposeTemplate[]>("remove_compose_template", { id }),
};
