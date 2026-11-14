<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import {
  NButton,
  NButtonGroup,
  NEmpty,
  NIcon,
  NInput,
  NPopconfirm,
  NProgress,
  NSelect,
  NSpin,
  NTable,
  NTag,
  NTooltip,
} from "naive-ui";
import {
  Activity,
  Anchor,
  Box,
  Cpu,
  Grid3X3,
  List,
  MemoryStick,
  Play,
  Plus,
  RefreshCw,
  RotateCw,
  ScrollText,
  Search,
  Ship,
  Square,
  TerminalSquare,
  Trash2,
} from "lucide-vue-next";
import ContainerDetailsDrawer from "../../components/ContainerDetailsDrawer.vue";
import CreateContainerModal from "../../components/CreateContainerModal.vue";
import { dict } from "../../dict";
import { useContainerStore, type ContainerAction } from "../../store/container";
import { useFleetStore } from "../../store/fleet";
import type { ContainerInfo } from "../../types";

const fleet = useFleetStore();
const containers = useContainerStore();
const ready = ref(false);
const search = ref("");
const stateFilter = ref("all");
const viewMode = ref<"card" | "table">(localStorage.getItem("shipwiz.containers.view") === "table" ? "table" : "card");
watch(viewMode, (value) => localStorage.setItem("shipwiz.containers.view", value));
const createVisible = ref(false);
const detailsVisible = ref(false);
const selected = ref<ContainerInfo | null>(null);
const detailsTab = ref<"logs" | "exec">("logs");
const currentNodeId = computed(() => fleet.state.currentNodeId);
const currentNode = computed(() => fleet.currentNode);
const stateLabels: Record<string, string> = { ...dict.container.states };
const stateOptions = computed(() => [
  { label: dict.container.allStates, value: "all" },
  ...Object.entries(stateLabels).map(([value, label]) => ({ value, label })),
]);
const filteredContainers = computed(() => {
  const keyword = search.value.trim().toLowerCase();
  return containers.items.filter((container) => {
    const matchesState = stateFilter.value === "all" || container.state === stateFilter.value;
    const matchesKeyword = !keyword || [container.name, container.image, container.shortId, container.status]
      .some((value) => value.toLowerCase().includes(keyword));
    return matchesState && matchesKeyword;
  });
});
const runningCount = computed(() => containers.items.filter((item) => item.state === "running").length);
const stoppedCount = computed(() => Math.max(containers.items.length - runningCount.value, 0));
const totalMemory = computed(() =>
  containers.items.reduce((total, item) => total + (item.state === "running" ? item.memoryUsage : 0), 0),
);
const emptyDescription = computed(() => {
  if (containers.failed) return dict.container.loadFailed;
  if (search.value || stateFilter.value !== "all") return dict.container.emptyFiltered;
  return dict.container.emptyDescription;
});

onMounted(async () => {
  if (!fleet.state.nodes.length) await fleet.load();
  ready.value = true;
  if (currentNodeId.value) await containers.load(currentNodeId.value);
});
watch(currentNodeId, async (nodeId, previousNodeId) => {
  if (!ready.value || !nodeId || nodeId === previousNodeId) return;
  detailsVisible.value = false;
  await containers.load(nodeId);
});

function formatBytes(bytes: number) {
  if (!bytes) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const index = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  const value = bytes / 1024 ** index;
  return `${value.toFixed(index === 0 ? 0 : value >= 100 ? 0 : 1)} ${units[index]}`;
}
function memoryPercent(container: ContainerInfo) {
  return container.memoryLimit > 0 ? Math.min((container.memoryUsage / container.memoryLimit) * 100, 100) : 0;
}
function stateType(state: string): "success" | "warning" | "error" | "info" | "default" {
  if (state === "running") return "success";
  if (["restarting", "created", "paused"].includes(state)) return "warning";
  if (["dead", "removing"].includes(state)) return "error";
  if (state === "exited") return "default";
  return "info";
}
function portSummary(container: ContainerInfo) {
  if (!container.ports.length) return dict.container.noPorts;
  const ports = container.ports.slice(0, 2).map((port) =>
    port.publicPort
      ? `${port.publicPort}:${port.privatePort}/${port.protocol}`
      : `${port.privatePort}/${port.protocol}`,
  );
  if (container.ports.length > 2) ports.push(`+${container.ports.length - 2}`);
  return ports.join("  ·  ");
}
function canStart(container: ContainerInfo) {
  return ["created", "exited"].includes(container.state);
}
function isAction(action: ContainerAction, container: ContainerInfo) {
  return containers.actionKey === `${action}:${container.id}`;
}
async function perform(action: ContainerAction, container: ContainerInfo) {
  if (!currentNodeId.value) return;
  await containers.act(action, currentNodeId.value, container.id);
}
function openDetails(container: ContainerInfo, tab: "logs" | "exec") {
  selected.value = container;
  detailsTab.value = tab;
  detailsVisible.value = true;
}
async function refresh() {
  if (currentNodeId.value) await containers.load(currentNodeId.value);
}
</script>

<template>
  <section class="container-page page page-enter">
    <header class="page-header">
      <div>
        <span class="eyebrow">{{ dict.container.eyebrow }}</span>
        <h1>{{ dict.container.title }}</h1>
        <p>{{ dict.container.description }}</p>
      </div>
      <n-button type="primary" size="large" :disabled="!currentNode" @click="createVisible = true">
        <template #icon><n-icon><Plus /></n-icon></template>
        {{ dict.container.create }}
      </n-button>
    </header>

    <div class="current-ship-strip">
      <div class="ship-emblem"><Ship :size="21" /></div>
      <div class="ship-copy">
        <span>{{ dict.container.currentShip }}</span>
        <strong>{{ currentNode?.name ?? "—" }}</strong>
      </div>
      <code>{{ currentNode?.type === "local" ? dict.fleet.local : `${currentNode?.host}:${currentNode?.port}` }}</code>
      <span class="connection-state" :class="currentNode?.status">
        <i />{{ currentNode?.status === "online" ? dict.fleet.online : dict.common.offline }}
      </span>
      <n-button quaternary circle :loading="containers.loading" :aria-label="dict.common.refresh" @click="refresh">
        <template #icon><n-icon><RefreshCw /></n-icon></template>
      </n-button>
    </div>

    <div class="container-stats">
      <div class="metric-card">
        <span class="metric-icon teal"><Box :size="18" /></span>
        <div><span>{{ dict.container.total }}</span><strong>{{ containers.items.length.toString().padStart(2, "0") }}</strong></div>
      </div>
      <div class="metric-card">
        <span class="metric-icon green"><Activity :size="18" /></span>
        <div><span>{{ dict.container.running }}</span><strong>{{ runningCount.toString().padStart(2, "0") }}</strong></div>
      </div>
      <div class="metric-card">
        <span class="metric-icon slate"><Anchor :size="18" /></span>
        <div><span>{{ dict.container.stopped }}</span><strong>{{ stoppedCount.toString().padStart(2, "0") }}</strong></div>
      </div>
      <div class="metric-card">
        <span class="metric-icon blue"><MemoryStick :size="18" /></span>
        <div><span>{{ dict.container.memory }}</span><strong class="memory-value">{{ formatBytes(totalMemory) }}</strong></div>
      </div>
    </div>

    <div class="container-panel">
      <div class="list-toolbar">
        <div class="list-title"><Box :size="18" /><h2>{{ dict.container.listTitle }}</h2></div>
        <n-button-group class="view-switch">
          <n-button size="small" :type="viewMode === 'card' ? 'primary' : 'default'" :ghost="viewMode === 'card'" aria-label="卡片视图" @click="viewMode = 'card'"><template #icon><n-icon :component="Grid3X3" /></template></n-button>
          <n-button size="small" :type="viewMode === 'table' ? 'primary' : 'default'" :ghost="viewMode === 'table'" aria-label="表格视图" @click="viewMode = 'table'"><template #icon><n-icon :component="List" /></template></n-button>
        </n-button-group>
        <n-input v-model:value="search" class="search-input" :placeholder="dict.container.searchPlaceholder" clearable>
          <template #prefix><n-icon><Search /></n-icon></template>
        </n-input>
        <n-select v-model:value="stateFilter" class="state-select" :options="stateOptions" />
        <n-button secondary :loading="containers.loading" @click="refresh">
          <template #icon><n-icon><RefreshCw /></n-icon></template>
          {{ dict.common.refresh }}
        </n-button>
      </div>

      <n-spin :show="containers.loading">
        <div v-if="filteredContainers.length && viewMode === 'table'" class="table-scroll">
          <n-table :single-line="false" class="container-table">
            <thead>
              <tr>
                <th>{{ dict.container.table.container }}</th>
                <th>{{ dict.container.table.state }}</th>
                <th>{{ dict.container.table.image }}</th>
                <th>{{ dict.container.table.cpu }}</th>
                <th>{{ dict.container.table.memory }}</th>
                <th>{{ dict.container.table.actions }}</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="container in filteredContainers" :key="container.id">
                <td>
                  <div class="container-identity">
                    <span class="container-cube"><Box :size="17" /></span>
                    <div><strong>{{ container.name }}</strong><code>{{ container.shortId }}</code></div>
                  </div>
                  <div class="port-summary" :title="portSummary(container)">{{ portSummary(container) }}</div>
                </td>
                <td>
                  <n-tag size="small" :bordered="false" :type="stateType(container.state)">
                    {{ stateLabels[container.state] ?? stateLabels.unknown }}
                  </n-tag>
                  <div class="status-detail" :title="container.status">{{ container.status }}</div>
                </td>
                <td><span class="image-name" :title="container.image">{{ container.image }}</span></td>
                <td>
                  <template v-if="container.state === 'running' && container.statsAvailable">
                    <div class="resource-value"><Cpu :size="12" />{{ container.cpuPercent.toFixed(1) }}%</div>
                    <n-progress type="line" :percentage="Math.min(container.cpuPercent, 100)" :show-indicator="false" :height="4" color="#1c9b88" />
                  </template>
                  <span v-else class="no-stats">—</span>
                </td>
                <td>
                  <template v-if="container.state === 'running' && container.statsAvailable">
                    <div class="memory-numbers">
                      <strong>{{ formatBytes(container.memoryUsage) }}</strong>
                      <span>/ {{ formatBytes(container.memoryLimit) }}</span>
                    </div>
                    <n-progress type="line" :percentage="memoryPercent(container)" :show-indicator="false" :height="4" color="#4a829c" />
                  </template>
                  <span v-else class="no-stats">—</span>
                </td>
                <td>
                  <div class="row-actions">
                    <n-tooltip v-if="canStart(container)">
                      <template #trigger>
                        <n-button quaternary circle size="small" class="start-action" :loading="isAction('start', container)" @click="perform('start', container)">
                          <template #icon><n-icon><Play /></n-icon></template>
                        </n-button>
                      </template>
                      {{ dict.container.actions.start }}
                    </n-tooltip>
                    <n-tooltip v-if="container.state === 'running'">
                      <template #trigger>
                        <n-button quaternary circle size="small" :loading="isAction('stop', container)" @click="perform('stop', container)">
                          <template #icon><n-icon><Square /></n-icon></template>
                        </n-button>
                      </template>
                      {{ dict.container.actions.stop }}
                    </n-tooltip>
                    <n-tooltip v-if="container.state === 'running'">
                      <template #trigger>
                        <n-button quaternary circle size="small" :loading="isAction('restart', container)" @click="perform('restart', container)">
                          <template #icon><n-icon><RotateCw /></n-icon></template>
                        </n-button>
                      </template>
                      {{ dict.container.actions.restart }}
                    </n-tooltip>
                    <n-tooltip>
                      <template #trigger>
                        <n-button quaternary circle size="small" @click="openDetails(container, 'logs')">
                          <template #icon><n-icon><ScrollText /></n-icon></template>
                        </n-button>
                      </template>
                      {{ dict.container.actions.logs }}
                    </n-tooltip>
                    <n-tooltip>
                      <template #trigger>
                        <n-button quaternary circle size="small" :disabled="container.state !== 'running'" @click="openDetails(container, 'exec')">
                          <template #icon><n-icon><TerminalSquare /></n-icon></template>
                        </n-button>
                      </template>
                      {{ dict.container.actions.exec }}
                    </n-tooltip>
                    <n-popconfirm
                      :positive-text="dict.common.delete"
                      :negative-text="dict.common.cancel"
                      @positive-click="perform('remove', container)"
                    >
                      <template #trigger>
                        <n-button quaternary circle size="small" type="error" :loading="isAction('remove', container)">
                          <template #icon><n-icon><Trash2 /></n-icon></template>
                        </n-button>
                      </template>
                      <strong>{{ dict.container.removeTitle }}</strong>
                      <div class="remove-hint">{{ container.name }} · {{ dict.container.removeDescription }}</div>
                    </n-popconfirm>
                  </div>
                </td>
              </tr>
            </tbody>
          </n-table>
        </div>
        <div v-if="filteredContainers.length && viewMode === 'card'" class="container-card-grid">
          <article v-for="container in filteredContainers" :key="container.id" class="container-card">
            <div class="container-card-head">
              <span class="container-cube"><Box :size="18" /></span>
              <div><strong>{{ container.name }}</strong><code>{{ container.shortId }}</code></div>
              <n-tag size="small" :bordered="false" :type="stateType(container.state)">{{ stateLabels[container.state] ?? stateLabels.unknown }}</n-tag>
            </div>
            <div class="container-card-image" :title="container.image">{{ container.image }}</div>
            <div class="container-card-port" :title="portSummary(container)">{{ portSummary(container) }}</div>
            <div v-if="container.state === 'running' && container.statsAvailable" class="container-card-metrics">
              <div><span><Cpu :size="12" />处理器 {{ container.cpuPercent.toFixed(1) }}%</span><n-progress type="line" :percentage="Math.min(container.cpuPercent, 100)" :show-indicator="false" :height="4" color="#165DFF" /></div>
              <div><span><MemoryStick :size="12" />{{ formatBytes(container.memoryUsage) }}</span><n-progress type="line" :percentage="memoryPercent(container)" :show-indicator="false" :height="4" color="#22C55E" /></div>
            </div>
            <div v-else class="container-card-idle">{{ container.status }}</div>
            <div class="container-card-actions">
              <n-button v-if="canStart(container)" size="small" type="primary" ghost :loading="isAction('start', container)" @click="perform('start', container)"><template #icon><n-icon :component="Play" /></template>{{ dict.container.actions.start }}</n-button>
              <n-button v-if="container.state === 'running'" size="small" :loading="isAction('stop', container)" @click="perform('stop', container)"><template #icon><n-icon :component="Square" /></template>{{ dict.container.actions.stop }}</n-button>
              <n-button v-if="container.state === 'running'" size="small" quaternary :loading="isAction('restart', container)" @click="perform('restart', container)"><template #icon><n-icon :component="RotateCw" /></template>{{ dict.container.actions.restart }}</n-button>
              <n-button size="small" quaternary @click="openDetails(container, 'logs')"><template #icon><n-icon :component="ScrollText" /></template>{{ dict.container.actions.logs }}</n-button>
              <n-button size="small" quaternary :disabled="container.state !== 'running'" @click="openDetails(container, 'exec')"><template #icon><n-icon :component="TerminalSquare" /></template>{{ dict.container.actions.exec }}</n-button>
              <n-popconfirm :positive-text="dict.common.delete" :negative-text="dict.common.cancel" @positive-click="perform('remove', container)"><template #trigger><n-button quaternary circle size="small" type="error" :loading="isAction('remove', container)"><template #icon><n-icon :component="Trash2" /></template></n-button></template><strong>{{ dict.container.removeTitle }}</strong><div class="remove-hint">{{ container.name }} · {{ dict.container.removeDescription }}</div></n-popconfirm>
            </div>
          </article>
        </div>
        <n-empty v-if="!filteredContainers.length" :description="emptyDescription" class="container-empty">
          <template #icon><Box :size="44" stroke-width="1.2" /></template>
          <template v-if="!containers.failed && !search && stateFilter === 'all'" #extra>
            <n-button type="primary" @click="createVisible = true">
              <template #icon><n-icon><Plus /></n-icon></template>
              {{ dict.container.create }}
            </n-button>
          </template>
        </n-empty>
      </n-spin>
    </div>

    <CreateContainerModal v-model:show="createVisible" :node-id="currentNodeId ?? ''" />
    <ContainerDetailsDrawer
      v-model:show="detailsVisible"
      :container="selected"
      :node-id="currentNodeId ?? ''"
      :initial-tab="detailsTab"
    />
  </section>
</template>

<style scoped>
.current-ship-strip { display: flex; align-items: center; gap: 13px; min-height: 64px; padding: 10px 16px; margin-bottom: 18px; border: 1px solid var(--sw-border); border-radius: 8px; background: var(--sw-surface); }
.ship-emblem { display: grid; place-items: center; width: 38px; height: 38px; border-radius: 8px; background: var(--sw-primary); color: var(--sw-terminal-text); }
.ship-copy { display: flex; flex-direction: column; min-width: 120px; }
.ship-copy span { color: var(--sw-muted); font-size: 9px; }
.ship-copy strong { margin-top: 3px; color: var(--sw-text); font-size: 13px; font-weight: 500; }
.current-ship-strip > code { margin-left: 5px; color: var(--sw-muted); font-size: 10px; }
.connection-state { display: flex; align-items: center; gap: 6px; margin-left: auto; color: var(--sw-muted); font-size: 11px; }
.connection-state i { width: 6px; height: 6px; border-radius: 50%; background: var(--sw-muted); }
.connection-state.online { color: var(--sw-success); }
.connection-state.online i { background: var(--sw-success); box-shadow: 0 0 0 4px var(--sw-success-soft); }
.container-stats { display: grid; grid-template-columns: repeat(4, 1fr); gap: 13px; margin-bottom: 18px; }
.metric-card { display: flex; align-items: center; gap: 14px; min-height: 84px; padding: 16px; border: 1px solid var(--sw-border); border-radius: 8px; background: var(--sw-surface); }
.metric-icon { display: grid; place-items: center; width: 36px; height: 36px; border-radius: 8px; }
.metric-icon.teal { background: var(--sw-primary-soft); color: var(--sw-primary); }
.metric-icon.green { background: var(--sw-success-soft); color: var(--sw-success); }
.metric-icon.slate { background: var(--sw-surface-soft); color: var(--sw-text-secondary); }
.metric-icon.blue { background: var(--sw-primary-soft); color: var(--sw-primary); }
.metric-card > div { display: flex; flex-direction: column; }
.metric-card span:not(.metric-icon) { color: var(--sw-muted); font-size: 10px; }
.metric-card strong { margin-top: 4px; color: var(--sw-text); font-family: "DIN Alternate", Consolas, sans-serif; font-size: 24px; font-weight: 500; line-height: 1; }
.metric-card strong.memory-value { font-size: 19px; }
.container-panel { overflow: hidden; border: 1px solid var(--sw-border); border-radius: 8px; background: var(--sw-surface); box-shadow: var(--sw-shadow); }
.list-toolbar { display: flex; align-items: center; gap: 10px; min-height: 67px; padding: 12px 16px; border-bottom: 1px solid var(--sw-border); }
.list-title { display: flex; align-items: center; gap: 9px; margin-right: auto; color: var(--sw-primary); }
.list-title h2 { margin: 0; color: var(--sw-text); font-size: 14px; font-weight: 500; }
.search-input { width: 245px; }
.state-select { width: 140px; }
.table-scroll { overflow-x: auto; }
.container-table { min-width: 1030px; font-size: 11px; }
.container-table :deep(th) { height: 40px; background: var(--sw-surface-soft); color: var(--sw-muted); font-size: 10px; font-weight: 400; }
.container-table :deep(td) { height: 82px; padding-top: 12px; padding-bottom: 12px; color: var(--sw-text-secondary); }
.container-table :deep(tbody tr) { transition: background 300ms ease; }
.container-table :deep(tbody tr:hover) { background: var(--sw-surface-soft); }
.container-table :deep(th:first-child) { width: 245px; }
.container-table :deep(th:nth-child(2)) { width: 125px; }
.container-table :deep(th:nth-child(3)) { width: 180px; }
.container-table :deep(th:nth-child(4)), .container-table :deep(th:nth-child(5)) { width: 135px; }
.container-table :deep(th:last-child) { width: 210px; }
.container-identity { display: flex; align-items: center; gap: 10px; }
.container-cube { display: grid; place-items: center; flex: 0 0 auto; width: 32px; height: 32px; border: 1px solid var(--sw-border); border-radius: 8px; background: var(--sw-primary-soft); color: var(--sw-primary); }
.container-identity > div { display: flex; flex-direction: column; min-width: 0; }
.container-identity strong { overflow: hidden; color: var(--sw-text); font-size: 12px; font-weight: 500; text-overflow: ellipsis; white-space: nowrap; }
.container-identity code { margin-top: 4px; color: var(--sw-muted); font-size: 9px; }
.port-summary { overflow: hidden; max-width: 205px; margin: 6px 0 0 42px; color: var(--sw-muted); font-family: Consolas, monospace; font-size: 9px; text-overflow: ellipsis; white-space: nowrap; }
.status-detail { overflow: hidden; max-width: 115px; margin-top: 7px; color: var(--sw-muted); font-size: 9px; text-overflow: ellipsis; white-space: nowrap; }
.image-name { display: block; overflow: hidden; max-width: 170px; color: var(--sw-text-secondary); font-family: Consolas, monospace; font-size: 10px; text-overflow: ellipsis; white-space: nowrap; }
.resource-value { display: flex; align-items: center; gap: 5px; margin-bottom: 7px; color: var(--sw-text-secondary); font-family: Consolas, monospace; font-size: 10px; }
.memory-numbers { display: flex; flex-direction: column; margin-bottom: 6px; }
.memory-numbers strong { color: var(--sw-text-secondary); font-size: 10px; font-weight: 500; }
.memory-numbers span { margin-top: 2px; color: var(--sw-muted); font-size: 9px; }
.no-stats { color: var(--sw-muted); }
.row-actions { display: flex; align-items: center; gap: 1px; }
.row-actions :deep(.n-button) { color: var(--sw-text-secondary); }
.row-actions :deep(.start-action) { color: var(--sw-primary); }
.remove-hint { max-width: 240px; margin-top: 6px; line-height: 1.6; }
.container-empty { min-height: 330px; justify-content: center; }
@media (max-width: 1100px) {
  .container-stats { grid-template-columns: repeat(2, 1fr); }
  .current-ship-strip > code { display: none; }
  .search-input { width: 200px; }
}
</style>
