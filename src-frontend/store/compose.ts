import { ref } from "vue";
import { defineStore } from "pinia";
import { createDiscreteApi } from "naive-ui";
import { composeApi } from "../api/compose";
import { dict } from "../dict";
import type { ComposeDeployment, ComposeProject, ComposeTemplate } from "../types";

const { message } = createDiscreteApi(["message"]);

export const useComposeStore = defineStore("compose", () => {
  const projects = ref<ComposeProject[]>([]);
  const templates = ref<ComposeTemplate[]>([]);
  const deployments = ref<ComposeDeployment[]>([]);
  const currentId = ref<string | null>(null);
  const loading = ref(false);
  const deploymentsLoading = ref(false);
  const action = ref<"up" | "down" | null>(null);
  const saving = ref(false);
  const lastOutput = ref("");

  async function loadLocal() {
    loading.value = true;
    try {
      const [projectList, templateList] = await Promise.all([
        composeApi.projects(),
        composeApi.templates(),
      ]);
      projects.value = projectList;
      templates.value = templateList;
      if (!currentId.value && projectList.length) currentId.value = projectList[0].id;
      return true;
    } catch {
      return false;
    } finally {
      loading.value = false;
    }
  }

  async function loadDeployments(nodeId: string) {
    deploymentsLoading.value = true;
    try {
      deployments.value = await composeApi.deployments(nodeId);
      return true;
    } catch {
      return false;
    } finally {
      deploymentsLoading.value = false;
    }
  }

  async function saveProject(name: string, yaml: string, id: string | null, quiet = false) {
    saving.value = true;
    try {
      const project = await composeApi.saveProject({ name, yaml }, id);
      const index = projects.value.findIndex((item) => item.id === project.id);
      if (index >= 0) projects.value[index] = project;
      else projects.value.unshift(project);
      currentId.value = project.id;
      if (!quiet) message.success(dict.compose.messages.saved);
      return project;
    } catch {
      return null;
    } finally {
      saving.value = false;
    }
  }

  async function removeProject(id: string) {
    saving.value = true;
    try {
      await composeApi.removeProject(id);
      projects.value = projects.value.filter((project) => project.id !== id);
      if (currentId.value === id) currentId.value = projects.value[0]?.id ?? null;
      message.success(dict.compose.messages.removed);
      return true;
    } catch {
      return false;
    } finally {
      saving.value = false;
    }
  }

  async function validate(yaml: string) {
    try {
      return await composeApi.validate(yaml);
    } catch {
      return null;
    }
  }

  async function up(nodeId: string, projectId: string) {
    action.value = "up";
    try {
      const result = await composeApi.up(nodeId, projectId);
      lastOutput.value = result.output;
      message.success(dict.compose.messages.started);
      await loadDeployments(nodeId);
      return true;
    } catch {
      return false;
    } finally {
      action.value = null;
    }
  }

  async function down(nodeId: string, projectId: string) {
    action.value = "down";
    try {
      const result = await composeApi.down(nodeId, projectId);
      lastOutput.value = result.output;
      message.success(dict.compose.messages.stopped);
      await loadDeployments(nodeId);
      return true;
    } catch {
      return false;
    } finally {
      action.value = null;
    }
  }

  async function saveTemplate(input: { title: string; desc: string; tag: string[]; yaml: string }) {
    saving.value = true;
    try {
      templates.value = await composeApi.saveTemplate(input, null);
      message.success(dict.compose.messages.templateSaved);
      return true;
    } catch {
      return false;
    } finally {
      saving.value = false;
    }
  }

  async function removeTemplate(id: string) {
    saving.value = true;
    try {
      templates.value = await composeApi.removeTemplate(id);
      message.success(dict.compose.messages.templateRemoved);
      return true;
    } catch {
      return false;
    } finally {
      saving.value = false;
    }
  }

  return {
    projects,
    templates,
    deployments,
    currentId,
    loading,
    deploymentsLoading,
    action,
    saving,
    lastOutput,
    loadLocal,
    loadDeployments,
    saveProject,
    removeProject,
    validate,
    up,
    down,
    saveTemplate,
    removeTemplate,
  };
});
