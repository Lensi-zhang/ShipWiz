<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import {
  NButton,
  NEmpty,
  NIcon,
  NInput,
  NModal,
  NPopconfirm,
  NSelect,
  NSpin,
  NTable,
  NTabPane,
  NTabs,
  NTag,
  NTooltip,
} from "naive-ui";
import { alertError, hoistSuccess, steerSuccess } from "../../utils/message";
import {
  BookOpen,
  CheckCircle2,
  FileCode2,
  FilePlus2,
  Layers3,
  Play,
  Plus,
  RefreshCw,
  Rocket,
  Save,
  ScrollText,
  Ship,
  Square,
  Trash2,
} from "lucide-vue-next";
import ComposeEditor from "../../components/ComposeEditor.vue";
import { composeApi } from "../../api/compose";
import { dict } from "../../dict";
import { useComposeStore } from "../../store/compose";
import { useFleetStore } from "../../store/fleet";
import type { ComposeProject, ComposeTemplate } from "../../types";

const fleet = useFleetStore();
const compose = useComposeStore();
const ready = ref(false);
const projectName = ref("new-fleet");
const yaml = ref("");
const editorErrors = ref(0);
const validatedServices = ref<string[]>([]);
const activeBottomTab = ref("deployments");
const templateVisible = ref(false);
const templateForm = ref({ title: "", desc: "", tags: "" });
const logsVisible = ref(false);
const logsLoading = ref(false);
const logsProject = ref("");
const logsContent = ref("");
const logTail = ref(200);
const defaultYaml = `services:
  web:
    image: nginx:1.27-alpine
    ports:
      - "8080:80"
    restart: unless-stopped
`;
const currentNodeId = computed(() => fleet.state.currentNodeId);
const currentNode = computed(() => fleet.currentNode);
const currentProject = computed(
  () =>
    compose.projects.find((project) => project.id === compose.currentId) ??
    null,
);
const dirty = computed(
  () =>
    !currentProject.value ||
    currentProject.value.name !== projectName.value ||
    currentProject.value.yaml !== yaml.value.trim(),
);
const builtInTemplates = computed(() =>
  compose.templates.filter((template) => template.builtIn),
);
const customTemplates = computed(() =>
  compose.templates.filter((template) => !template.builtIn),
);
const stateLabels: Record<string, string> = { ...dict.compose.states };

onMounted(async () => {
  const tasks: Promise<unknown>[] = [compose.loadLocal()];
  if (!fleet.state.nodes.length) tasks.push(fleet.load());
  await Promise.all(tasks);
  ready.value = true;
  if (compose.currentId) syncCurrentProject();
  else resetNewProject();
  if (currentNodeId.value) await compose.loadDeployments(currentNodeId.value);
});

watch(
  () => compose.currentId,
  (id) => {
    if (ready.value && id) syncCurrentProject();
  },
);
watch(currentNodeId, async (nodeId, previousNodeId) => {
  if (!ready.value || !nodeId || nodeId === previousNodeId) return;
  logsVisible.value = false;
  await compose.loadDeployments(nodeId);
});
watch(yaml, () => {
  validatedServices.value = [];
  compose.lastOutput = "";
});

function syncCurrentProject() {
  if (!currentProject.value) return;
  projectName.value = currentProject.value.name;
  yaml.value = currentProject.value.yaml;
  validatedServices.value = [];
  compose.lastOutput = "";
}
function uniqueProjectName(value: string) {
  const base =
    value
      .toLowerCase()
      .replace(/[^a-z0-9_-]+/g, "-")
      .replace(/^[^a-z0-9]+/, "")
      .slice(0, 50) || "fleet";
  let candidate = base;
  let suffix = 2;
  while (compose.projects.some((project) => project.name === candidate)) {
    candidate = `${base}-${suffix}`;
    suffix += 1;
  }
  return candidate;
}
function resetNewProject() {
  compose.currentId = null;
  projectName.value = uniqueProjectName("new-fleet");
  yaml.value = defaultYaml;
  editorErrors.value = 0;
  validatedServices.value = [];
  compose.lastOutput = "";
}
async function saveCurrent(quiet = false) {
  const project = await compose.saveProject(
    projectName.value,
    yaml.value,
    compose.currentId,
    quiet,
  );
  if (project) {
    projectName.value = project.name;
    yaml.value = project.yaml;
  }
  return project;
}
async function removeProject(project: ComposeProject) {
  if (await compose.removeProject(project.id)) {
    if (compose.currentId) syncCurrentProject();
    else resetNewProject();
  }
}
async function validateCurrent() {
  const result = await compose.validate(yaml.value);
  if (result) {
    validatedServices.value = result.services;
    steerSuccess(dict.compose.validationSuccess);
  }
}
async function startCurrent() {
  if (!currentNodeId.value) return;
  const project = await saveCurrent(true);
  if (project) await compose.up(currentNodeId.value, project.id);
}
async function startSaved(projectId: string) {
  if (currentNodeId.value) await compose.up(currentNodeId.value, projectId);
}
async function stopProject(projectId: string) {
  if (currentNodeId.value) await compose.down(currentNodeId.value, projectId);
}
function projectForDeployment(name: string) {
  return compose.projects.find((project) => project.name === name) ?? null;
}
function stateType(state: string): "success" | "warning" | "default" {
  if (state === "running") return "success";
  if (state === "partial") return "warning";
  return "default";
}
function stateLabel(state: string) {
  return stateLabels[state] ?? state;
}
function formatDate(timestamp: number) {
  return new Intl.DateTimeFormat("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(timestamp));
}
function openTemplateModal() {
  templateForm.value = {
    title: `${projectName.value} 模板`,
    desc: "",
    tags: "",
  };
  templateVisible.value = true;
}
async function submitTemplate() {
  if (!templateForm.value.title.trim()) {
    alertError(dict.compose.templateModal.required);
    return;
  }
  const saved = await compose.saveTemplate({
    title: templateForm.value.title,
    desc: templateForm.value.desc,
    tag: templateForm.value.tags
      .split(/[,，]/)
      .map((tag) => tag.trim())
      .filter(Boolean),
    yaml: yaml.value,
  });
  if (saved) templateVisible.value = false;
}
function loadTemplate(template: ComposeTemplate) {
  compose.currentId = null;
  const suggestedName = template.builtIn
    ? `${template.id.replace("builtin-", "")}-fleet`
    : template.title;
  projectName.value = uniqueProjectName(suggestedName);
  yaml.value = template.yaml;
  validatedServices.value = [];
  compose.lastOutput = "";
  hoistSuccess(dict.compose.messages.templateLoaded);
}
async function openLogs(project: string) {
  logsProject.value = project;
  logsContent.value = "";
  logsVisible.value = true;
  await loadLogs();
}
async function loadLogs() {
  if (!currentNodeId.value || !logsProject.value) return;
  logsLoading.value = true;
  try {
    logsContent.value = await composeApi.logs(
      currentNodeId.value,
      logsProject.value,
      logTail.value,
    );
  } catch {
    logsContent.value = "";
  } finally {
    logsLoading.value = false;
  }
}
</script>

<template>
  <section class="compose-page page page-enter">
    <header class="page-header">
      <div class="page-heading">
        <span class="eyebrow">{{ dict.compose.eyebrow }}</span>
        <h1>{{ dict.compose.title }}</h1>
        <p>{{ dict.compose.description }}</p>
      </div>
      <div class="header-actions">
        <n-button size="large" @click="resetNewProject">
          <template #icon
            ><n-icon><FilePlus2 /></n-icon
          ></template>
          {{ dict.compose.new }}
        </n-button>
        <n-button
          type="primary"
          size="large"
          :disabled="!currentNode"
          :loading="compose.action === 'up'"
          @click="startCurrent"
        >
          <template #icon
            ><n-icon><Rocket /></n-icon
          ></template>
          {{ compose.action === "up" ? dict.compose.running : dict.compose.up }}
        </n-button>
      </div>
    </header>

    <div class="current-ship-strip">
      <div class="ship-emblem"><Ship :size="21" /></div>
      <div class="ship-copy">
        <span>{{ dict.compose.currentShip }}</span>
        <strong>{{ currentNode?.name ?? "—" }}</strong>
      </div>
      <code>{{
        currentNode?.type === "local"
          ? dict.fleet.local
          : `${currentNode?.host}:${currentNode?.port}`
      }}</code>
      <span class="connection-state" :class="currentNode?.status">
        <i />{{
          currentNode?.status === "online"
            ? dict.fleet.online
            : dict.common.offline
        }}
      </span>
    </div>

    <div class="compose-workspace">
      <aside class="project-panel compose-panel">
        <div class="panel-heading">
          <div>
            <Layers3 :size="17" />
            <h2>{{ dict.compose.projects }}</h2>
          </div>
          <n-tag size="small" :bordered="false">{{
            compose.projects.length.toString().padStart(2, "0")
          }}</n-tag>
        </div>
        <n-spin :show="compose.loading">
          <div v-if="compose.projects.length" class="project-list">
            <article
              v-for="project in compose.projects"
              :key="project.id"
              class="project-item"
              :class="{ active: compose.currentId === project.id }"
              tabindex="0"
              @click="compose.currentId = project.id"
              @keydown.enter="compose.currentId = project.id"
            >
              <span class="project-file"><FileCode2 :size="17" /></span>
              <div class="project-copy">
                <strong>{{ project.name }}</strong>
                <span
                  >{{ dict.compose.updated }} ·
                  {{ formatDate(project.updatedTime) }}</span
                >
              </div>
              <n-popconfirm
                :positive-text="dict.common.delete"
                :negative-text="dict.common.cancel"
                @positive-click="removeProject(project)"
              >
                <template #trigger>
                  <n-button
                    quaternary
                    circle
                    size="tiny"
                    type="error"
                    :loading="compose.saving"
                    @click.stop
                  >
                    <template #icon
                      ><n-icon><Trash2 /></n-icon
                    ></template>
                  </n-button>
                </template>
                <strong>{{ dict.compose.removeTitle }}</strong>
                <p class="confirm-copy">{{ dict.compose.removeDescription }}</p>
              </n-popconfirm>
            </article>
          </div>
          <n-empty
            v-else
            :description="dict.compose.noProjects"
            class="project-empty"
          >
            <template #icon><Layers3 :size="38" stroke-width="1.2" /></template>
          </n-empty>
        </n-spin>
        <n-button
          block
          dashed
          class="new-project-button"
          @click="resetNewProject"
        >
          <template #icon
            ><n-icon><Plus /></n-icon
          ></template>
          {{ dict.compose.new }}
        </n-button>
      </aside>

      <div class="editor-panel compose-panel">
        <div class="editor-toolbar">
          <n-input
            v-model:value="projectName"
            class="project-name"
            :placeholder="dict.compose.namePlaceholder"
          >
            <template #prefix
              ><n-icon><FileCode2 /></n-icon
            ></template>
          </n-input>
          <div class="editor-actions">
            <n-button quaternary @click="openTemplateModal">
              <template #icon
                ><n-icon><BookOpen /></n-icon
              ></template>
              {{ dict.compose.saveTemplate }}
            </n-button>
            <n-button secondary @click="validateCurrent">
              <template #icon
                ><n-icon><CheckCircle2 /></n-icon
              ></template>
              {{ dict.compose.validate }}
            </n-button>
            <n-button
              secondary
              type="primary"
              :loading="compose.saving"
              @click="saveCurrent()"
            >
              <template #icon
                ><n-icon><Save /></n-icon
              ></template>
              {{ dict.compose.save }}
            </n-button>
            <n-popconfirm
              v-if="currentProject"
              :positive-text="dict.compose.down"
              :negative-text="dict.common.cancel"
              @positive-click="stopProject(currentProject.id)"
            >
              <template #trigger>
                <n-button
                  type="error"
                  secondary
                  :loading="compose.action === 'down'"
                >
                  <template #icon
                    ><n-icon><Square /></n-icon
                  ></template>
                  {{ dict.compose.down }}
                </n-button>
              </template>
              <strong>{{ dict.compose.downTitle }}</strong>
              <p class="confirm-copy">{{ dict.compose.downDescription }}</p>
            </n-popconfirm>
          </div>
        </div>
        <ComposeEditor
          v-model="yaml"
          class="editor-canvas"
          @validation="editorErrors = $event"
        />
        <div class="editor-status">
          <span :class="{ invalid: editorErrors > 0 }">
            <i />{{
              editorErrors > 0
                ? `${editorErrors} ${dict.compose.issues}`
                : dict.compose.yamlHint
            }}
          </span>
          <n-tag v-if="dirty" size="small" type="warning" :bordered="false">{{
            dict.compose.unsaved
          }}</n-tag>
          <div v-if="validatedServices.length" class="validated-services">
            <span>{{ dict.compose.serviceList }}</span>
            <n-tag
              v-for="service in validatedServices"
              :key="service"
              size="small"
              :bordered="false"
              type="success"
              >{{ service }}</n-tag
            >
          </div>
        </div>
        <div v-if="compose.lastOutput" class="command-output">
          <span>{{ dict.compose.output }}</span>
          <pre>{{ compose.lastOutput }}</pre>
        </div>
      </div>
    </div>

    <div class="operations-panel compose-panel">
      <n-tabs v-model:value="activeBottomTab" type="line" animated>
        <n-tab-pane name="deployments" :tab="dict.compose.deployments">
          <div class="deployment-toolbar">
            <span>{{
              compose.deployments.length.toString().padStart(2, "0")
            }}</span>
            <n-button
              secondary
              size="small"
              :loading="compose.deploymentsLoading"
              @click="currentNodeId && compose.loadDeployments(currentNodeId)"
            >
              <template #icon
                ><n-icon><RefreshCw /></n-icon
              ></template>
              {{ dict.compose.refresh }}
            </n-button>
          </div>
          <n-spin :show="compose.deploymentsLoading">
            <div v-if="compose.deployments.length" class="table-scroll">
              <n-table :single-line="false" class="deployment-table">
                <thead>
                  <tr>
                    <th>{{ dict.compose.deployments }}</th>
                    <th>{{ dict.container.table.state }}</th>
                    <th>{{ dict.compose.serviceList }}</th>
                    <th>{{ dict.container.table.container }}</th>
                    <th>{{ dict.container.table.actions }}</th>
                  </tr>
                </thead>
                <tbody>
                  <tr
                    v-for="deployment in compose.deployments"
                    :key="deployment.name"
                  >
                    <td>
                      <strong class="deployment-name">{{
                        deployment.name
                      }}</strong>
                    </td>
                    <td>
                      <n-tag
                        size="small"
                        :bordered="false"
                        :type="stateType(deployment.state)"
                        >{{ stateLabel(deployment.state) }}</n-tag
                      >
                    </td>
                    <td>
                      <div class="service-tags">
                        <n-tag
                          v-for="service in deployment.services"
                          :key="service"
                          size="tiny"
                          :bordered="false"
                          >{{ service }}</n-tag
                        >
                      </div>
                    </td>
                    <td>
                      <span class="container-count"
                        >{{ deployment.running }} /
                        {{ deployment.containers }}</span
                      >
                    </td>
                    <td>
                      <div class="row-actions">
                        <n-tooltip>
                          <template #trigger
                            ><n-button
                              quaternary
                              circle
                              size="small"
                              @click="openLogs(deployment.name)"
                              ><template #icon
                                ><n-icon
                                  ><ScrollText /></n-icon></template></n-button
                          ></template>
                          {{ dict.compose.logs }}
                        </n-tooltip>
                        <template v-if="projectForDeployment(deployment.name)">
                          <n-tooltip v-if="deployment.state !== 'running'">
                            <template #trigger
                              ><n-button
                                quaternary
                                circle
                                size="small"
                                type="primary"
                                :loading="compose.action === 'up'"
                                @click="
                                  startSaved(
                                    projectForDeployment(deployment.name)!.id,
                                  )
                                "
                                ><template #icon
                                  ><n-icon
                                    ><Play /></n-icon></template></n-button
                            ></template>
                            {{ dict.compose.up }}
                          </n-tooltip>
                          <n-popconfirm
                            :positive-text="dict.compose.down"
                            :negative-text="dict.common.cancel"
                            @positive-click="
                              stopProject(
                                projectForDeployment(deployment.name)!.id,
                              )
                            "
                          >
                            <template #trigger
                              ><n-button
                                quaternary
                                circle
                                size="small"
                                type="error"
                                :loading="compose.action === 'down'"
                                ><template #icon
                                  ><n-icon
                                    ><Square /></n-icon></template></n-button
                            ></template>
                            <strong>{{ dict.compose.downTitle }}</strong>
                            <p class="confirm-copy">
                              {{ dict.compose.downDescription }}
                            </p>
                          </n-popconfirm>
                        </template>
                      </div>
                    </td>
                  </tr>
                </tbody>
              </n-table>
            </div>
            <n-empty
              v-else
              :description="dict.compose.noDeployments"
              class="operations-empty"
            >
              <template #icon
                ><Rocket :size="40" stroke-width="1.2"
              /></template>
            </n-empty>
          </n-spin>
        </n-tab-pane>

        <n-tab-pane name="templates" :tab="dict.compose.templates">
          <section class="template-section">
            <h3>{{ dict.compose.builtInTemplates }}</h3>
            <div class="template-grid">
              <article
                v-for="template in builtInTemplates"
                :key="template.id"
                class="template-card"
              >
                <span class="template-icon"><BookOpen :size="20" /></span>
                <h4>{{ template.title }}</h4>
                <p>{{ template.desc }}</p>
                <div class="service-tags">
                  <n-tag
                    v-for="tag in template.tag"
                    :key="tag"
                    size="tiny"
                    :bordered="false"
                    >{{ tag }}</n-tag
                  >
                </div>
                <n-button
                  secondary
                  type="primary"
                  size="small"
                  @click="loadTemplate(template)"
                  >{{ dict.compose.importTemplate }}</n-button
                >
              </article>
            </div>
          </section>
          <section class="template-section local-templates">
            <h3>{{ dict.compose.localTemplates }}</h3>
            <div v-if="customTemplates.length" class="template-grid">
              <article
                v-for="template in customTemplates"
                :key="template.id"
                class="template-card"
              >
                <span class="template-icon"><FileCode2 :size="20" /></span>
                <n-popconfirm
                  :positive-text="dict.common.delete"
                  :negative-text="dict.common.cancel"
                  @positive-click="compose.removeTemplate(template.id)"
                >
                  <template #trigger
                    ><n-button
                      class="template-delete"
                      quaternary
                      circle
                      size="tiny"
                      type="error"
                      ><template #icon
                        ><n-icon><Trash2 /></n-icon></template></n-button
                  ></template>
                  <strong>{{ dict.compose.removeTemplateTitle }}</strong>
                  <p class="confirm-copy">
                    {{ dict.compose.removeTemplateDescription }}
                  </p>
                </n-popconfirm>
                <h4>{{ template.title }}</h4>
                <p>{{ template.desc }}</p>
                <div class="service-tags">
                  <n-tag
                    v-for="tag in template.tag"
                    :key="tag"
                    size="tiny"
                    :bordered="false"
                    >{{ tag }}</n-tag
                  >
                </div>
                <n-button
                  secondary
                  type="primary"
                  size="small"
                  @click="loadTemplate(template)"
                  >{{ dict.compose.importTemplate }}</n-button
                >
              </article>
            </div>
            <n-empty
              v-else
              :description="dict.compose.emptyTemplates"
              class="local-template-empty"
            />
          </section>
        </n-tab-pane>
      </n-tabs>
    </div>

    <n-modal
      v-model:show="templateVisible"
      preset="card"
      :title="dict.compose.templateModal.title"
      class="form-modal template-modal"
    >
      <div class="template-form">
        <label
          >{{ dict.compose.templateModal.name
          }}<n-input
            v-model:value="templateForm.title"
            :placeholder="dict.compose.templateModal.namePlaceholder"
        /></label>
        <label
          >{{ dict.compose.templateModal.description
          }}<n-input
            v-model:value="templateForm.desc"
            type="textarea"
            :rows="3"
            :placeholder="dict.compose.templateModal.descriptionPlaceholder"
        /></label>
        <label
          >{{ dict.compose.templateModal.tags
          }}<n-input
            v-model:value="templateForm.tags"
            :placeholder="dict.compose.templateModal.tagsPlaceholder"
        /></label>
      </div>
      <template #footer>
        <div class="modal-actions">
          <n-button @click="templateVisible = false">{{
            dict.common.cancel
          }}</n-button><n-button
            type="primary"
            :loading="compose.saving"
            @click="submitTemplate"
            >{{ dict.compose.templateModal.submit }}</n-button
          >
        </div>
      </template>
    </n-modal>

    <n-modal
      v-model:show="logsVisible"
      preset="card"
      :title="`${dict.compose.logs} · ${logsProject}`"
      class="logs-modal"
    >
      <div class="logs-toolbar">
        <span>{{ dict.container.logs.tail }}</span>
        <n-select
          v-model:value="logTail"
          class="tail-select"
          :options="
            [100, 200, 500, 1000].map((value) => ({
              label: String(value),
              value,
            }))
          "
        />
        <n-button
          secondary
          size="small"
          :loading="logsLoading"
          @click="loadLogs"
          ><template #icon
            ><n-icon><RefreshCw /></n-icon></template
          >{{ dict.container.logs.refresh }}</n-button
        >
      </div>
      <n-spin :show="logsLoading">
        <pre v-if="logsContent.trim()" class="compose-logs">{{
          logsContent
        }}</pre>
        <n-empty
          v-else
          :description="dict.container.logs.empty"
          class="logs-empty"
          ><template #icon
            ><ScrollText :size="40" stroke-width="1.2" /></template
        ></n-empty>
      </n-spin>
    </n-modal>
  </section>
</template>

<style scoped>
.current-ship-strip {
  display: flex;
  align-items: center;
  gap: 13px;
  min-height: 64px;
  padding: 10px 16px;
  margin-bottom: 18px;
  border: 1px solid var(--sw-border);
  border-radius: 8px;
  background: var(--sw-surface);
}
.ship-emblem {
  display: grid;
  place-items: center;
  width: 38px;
  height: 38px;
  color: var(--sw-terminal-text);
  border-radius: 8px;
  background: var(--sw-primary);
}
.ship-copy {
  display: flex;
  flex-direction: column;
  min-width: 120px;
}
.ship-copy span {
  color: var(--sw-muted);
  font-size: 9px;
}
.ship-copy strong {
  margin-top: 3px;
  color: var(--sw-text);
  font-size: 13px;
  font-weight: 500;
}
.current-ship-strip > code {
  margin-left: 5px;
  color: var(--sw-muted);
  font-size: 10px;
}
.connection-state {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-left: auto;
  color: var(--sw-muted);
  font-size: 11px;
}
.connection-state i {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--sw-muted);
}
.connection-state.online {
  color: var(--sw-success);
}
.connection-state.online i {
  background: var(--sw-success);
  box-shadow: 0 0 0 4px var(--sw-success-soft);
}
.compose-workspace {
  display: grid;
  grid-template-columns: 265px minmax(0, 1fr);
  gap: 16px;
  align-items: start;
}
.compose-panel {
  border: 1px solid var(--sw-border);
  border-radius: 8px;
  background: var(--sw-surface);
  box-shadow: var(--sw-shadow);
}
.project-panel {
  padding: 16px;
}
.panel-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 0 12px;
}
.panel-heading > div {
  display: flex;
  align-items: center;
  gap: 8px;
  color: var(--sw-primary);
}
.panel-heading h2 {
  margin: 0;
  color: var(--sw-text);
  font-size: 12px;
  font-weight: 500;
}
.project-list {
  display: grid;
  gap: 6px;
  min-height: 80px;
}
.project-item {
  display: flex;
  align-items: center;
  gap: 9px;
  padding: 11px 8px;
  border: 1px solid transparent;
  border-radius: 8px;
  cursor: pointer;
  outline: none;
  transition:
    background 300ms,
    border-color 300ms;
}
.project-item:hover,
.project-item:focus {
  border-color: var(--sw-border);
  background: var(--sw-surface-soft);
}
.project-item.active {
  border-color: var(--sw-primary);
  background: var(--sw-primary-soft);
}
.project-file {
  display: grid;
  place-items: center;
  flex: 0 0 auto;
  width: 30px;
  height: 30px;
  color: var(--sw-primary);
  border-radius: 8px;
  background: var(--sw-primary-soft);
}
.project-copy {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-width: 0;
}
.project-copy strong {
  overflow: hidden;
  color: var(--sw-text);
  font-family: Consolas, monospace;
  font-size: 11px;
  font-weight: 500;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.project-copy span {
  margin-top: 4px;
  color: var(--sw-muted);
  font-size: 8px;
}
.project-empty {
  min-height: 270px;
  justify-content: center;
}
.new-project-button {
  margin-top: 12px;
  color: var(--sw-text-secondary);
}
.editor-panel {
  overflow: hidden;
}
.editor-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 16px;
}
.project-name {
  width: 250px;
}
.editor-actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 6px;
  flex-wrap: wrap;
}
.editor-canvas {
  height: 480px;
  border-top: 1px solid var(--sw-border);
  border-bottom: 1px solid var(--sw-border);
}
.editor-status {
  display: flex;
  align-items: center;
  gap: 9px;
  min-height: 45px;
  padding: 8px 16px;
  color: var(--sw-muted);
  font-size: 9px;
}
.editor-status > span {
  display: flex;
  align-items: center;
  gap: 6px;
}
.editor-status > span i {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--sw-success);
}
.editor-status > span.invalid {
  color: var(--sw-danger);
}
.editor-status > span.invalid i {
  background: var(--sw-danger);
}
.validated-services {
  display: flex;
  align-items: center;
  gap: 5px;
  margin-left: auto;
}
.command-output {
  padding: 0 16px 16px;
}
.command-output > span {
  color: var(--sw-text-secondary);
  font-size: 9px;
}
.command-output pre {
  overflow: auto;
  max-height: 150px;
  padding: 12px;
  margin: 7px 0 0;
  color: var(--sw-terminal-text);
  border-radius: 8px;
  background: var(--sw-terminal-bg);
  font:
    10px/1.65 Consolas,
    monospace;
  white-space: pre-wrap;
}
.operations-panel {
  margin-top: 16px;
  padding: 0 16px 16px;
}
.deployment-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 12px;
}
.deployment-toolbar > span {
  color: var(--sw-muted);
  font:
    18px Consolas,
    monospace;
}
.table-scroll {
  overflow-x: auto;
  margin: 0 -16px -16px;
}
.deployment-table {
  min-width: 760px;
  font-size: 11px;
}
.deployment-table :deep(th) {
  height: 40px;
  color: var(--sw-muted);
  background: var(--sw-surface-soft);
  font-size: 10px;
  font-weight: 400;
}
.deployment-table :deep(td) {
  height: 62px;
  color: var(--sw-text-secondary);
}
.deployment-table :deep(th:first-child),
.deployment-table :deep(td:first-child) {
  padding-left: 16px;
}
.deployment-name {
  color: var(--sw-text);
  font:
    11px Consolas,
    monospace;
}
.service-tags {
  display: flex;
  gap: 4px;
  flex-wrap: wrap;
}
.container-count {
  color: var(--sw-text-secondary);
  font:
    11px Consolas,
    monospace;
}
.row-actions {
  display: flex;
  gap: 3px;
}
.confirm-copy {
  max-width: 270px;
  margin: 6px 0 0;
  line-height: 1.6;
}
.operations-empty {
  min-height: 230px;
  justify-content: center;
}
.template-section h3 {
  margin: 4px 0 12px;
  color: var(--sw-text-secondary);
  font-size: 11px;
  font-weight: 500;
}
.template-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 11px;
}
.template-card {
  position: relative;
  display: flex;
  align-items: flex-start;
  flex-direction: column;
  min-height: 190px;
  padding: 16px;
  border: 1px solid var(--sw-border);
  border-radius: 8px;
  background: var(--sw-surface-soft);
}
.template-icon {
  display: grid;
  place-items: center;
  width: 36px;
  height: 36px;
  color: var(--sw-primary);
  border-radius: 8px;
  background: var(--sw-primary-soft);
}
.template-card h4 {
  margin: 12px 0 5px;
  color: var(--sw-text);
  font-size: 12px;
  font-weight: 500;
}
.template-card p {
  flex: 1;
  margin: 0 0 10px;
  color: var(--sw-muted);
  font-size: 10px;
  line-height: 1.65;
}
.template-card .service-tags {
  margin-bottom: 13px;
}
.template-delete {
  position: absolute;
  top: 10px;
  right: 10px;
}
.local-templates {
  padding-top: 18px;
  margin-top: 18px;
  border-top: 1px solid var(--sw-border);
}
.local-template-empty {
  min-height: 130px;
  justify-content: center;
}
.template-form {
  display: grid;
  gap: 15px;
}
.template-form label {
  display: grid;
  gap: 7px;
  color: var(--sw-text-secondary);
  font-size: 11px;
}
.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
.logs-toolbar {
  display: flex;
  align-items: center;
  gap: 9px;
  margin-bottom: 12px;
  color: var(--sw-text-secondary);
  font-size: 10px;
}
.tail-select {
  width: 95px;
}
.compose-logs {
  overflow: auto;
  height: 480px;
  padding: 16px;
  margin: 0;
  color: var(--sw-terminal-text);
  border-radius: 8px;
  background: var(--sw-terminal-bg);
  font:
    11px/1.7 Consolas,
    monospace;
  white-space: pre-wrap;
  word-break: break-word;
}
.logs-empty {
  height: 420px;
  justify-content: center;
}
:deep(.logs-modal) {
  width: min(820px, calc(100vw - 48px));
}
@media (max-width: 1180px) {
  .compose-workspace {
    grid-template-columns: 220px minmax(0, 1fr);
  }
  .editor-toolbar {
    align-items: stretch;
    flex-direction: column;
  }
  .project-name {
    width: 100%;
  }
  .editor-actions {
    justify-content: flex-start;
  }
  .template-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}
@media (max-width: 850px) {
  .compose-workspace {
    grid-template-columns: 1fr;
  }
  .project-panel {
    max-height: 300px;
    overflow-y: auto;
  }
  .current-ship-strip > code {
    display: none;
  }
  .template-grid {
    grid-template-columns: 1fr;
  }
}
</style>
