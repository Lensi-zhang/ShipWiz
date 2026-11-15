<script setup lang="ts">
import { computed, nextTick, onUnmounted, ref, watch } from "vue";
import {
  NAlert,
  NButton,
  NDrawer,
  NDrawerContent,
  NEmpty,
  NIcon,
  NSelect,
  NSpin,
  NSwitch,
  NTabPane,
  NTabs,
} from "naive-ui";
import {
  Download,
  RefreshCw,
  ScrollText,
  TerminalSquare,
} from "lucide-vue-next";
import { containerApi } from "../api/container";
import { dict } from "../dict";
import { useAppearanceStore } from "../store/appearance";
import type { ContainerInfo } from "../types";
import TerminalPanel from "./TerminalPanel.vue";

const props = defineProps<{
  show: boolean;
  container: ContainerInfo | null;
  nodeId: string;
  initialTab: "logs" | "exec";
}>();
const emit = defineEmits<{ "update:show": [value: boolean] }>();
const appearance = useAppearanceStore();
const visible = computed({
  get: () => props.show,
  set: (value) => emit("update:show", value),
});
const activeTab = ref<"logs" | "exec">("logs");
const logs = ref("");
const logsLoading = ref(false);
const follow = ref(appearance.logFollow);
const tail = ref(appearance.logTail);
const logOutput = ref<HTMLElement | null>(null);
let followTimer: ReturnType<typeof setInterval> | null = null;
const tailOptions = [100, 200, 500, 1000].map((value) => ({
  label: String(value),
  value,
}));

function stopFollowTimer() {
  if (followTimer) clearInterval(followTimer);
  followTimer = null;
}

function syncFollowTimer() {
  stopFollowTimer();
  if (visible.value && activeTab.value === "logs" && follow.value) {
    followTimer = setInterval(() => loadLogs(true), 3000);
  }
}

async function loadLogs(quiet = false) {
  if (!props.container || logsLoading.value) return;
  if (!quiet) logsLoading.value = true;
  try {
    logs.value = await containerApi.logs(
      props.nodeId,
      props.container.id,
      tail.value,
    );
    await nextTick();
    if (logOutput.value)
      logOutput.value.scrollTop = logOutput.value.scrollHeight;
  } catch {
    follow.value = false;
  } finally {
    logsLoading.value = false;
  }
}

watch(
  () => [props.show, props.container?.id] as const,
  ([show]) => {
    stopFollowTimer();
    if (!show) return;
    activeTab.value = props.initialTab;
    logs.value = "";
    follow.value = appearance.logFollow;
    void loadLogs();
    syncFollowTimer();
  },
);
watch(activeTab, (tab) => {
  if (tab === "logs" && !logs.value) void loadLogs();
  syncFollowTimer();
});
watch(follow, syncFollowTimer);
watch(tail, () => loadLogs());
onUnmounted(stopFollowTimer);

function exportLogs() {
  if (!logs.value || !props.container) return;
  const blob = new Blob([logs.value], { type: "text/plain;charset=utf-8" });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = `shipwiz-${props.container.name}-logs.txt`;
  anchor.click();
  URL.revokeObjectURL(url);
}
</script>

<template>
  <n-drawer
    v-model:show="visible"
    width="min(520px, 100vw)"
    placement="right"
    class="container-details"
  >
    <n-drawer-content closable>
      <template #header>
        <div class="drawer-title">
          <span>{{ container?.name }}</span>
          <code>{{ container?.shortId }}</code>
        </div>
      </template>

      <n-tabs v-model:value="activeTab" type="line" animated>
        <n-tab-pane name="logs" :tab="dict.container.logs.title">
          <div class="panel-intro">
            <div class="panel-heading">
              <ScrollText :size="18" />{{ dict.container.logs.title }}
            </div>
            <p>{{ dict.container.logs.subtitle }}</p>
          </div>
          <div class="log-toolbar">
            <n-select
              v-model:value="tail"
              :options="tailOptions"
              class="tail-select"
            />
            <label class="follow-toggle">
              <n-switch v-model:value="follow" size="small" />
              <span>{{ dict.container.logs.follow }}</span>
            </label>
            <n-button
              size="small"
              secondary
              :loading="logsLoading"
              @click="loadLogs()"
            >
              <template #icon
                ><n-icon><RefreshCw /></n-icon
              ></template>
              {{ dict.container.logs.refresh }}
            </n-button>
            <n-button
              size="small"
              quaternary
              :disabled="!logs"
              @click="exportLogs"
            >
              <template #icon
                ><n-icon><Download /></n-icon
              ></template>
              {{ dict.container.logs.export }}
            </n-button>
          </div>
          <n-spin :show="logsLoading && !logs">
            <div class="terminal log-terminal">
              <pre v-if="logs" ref="logOutput">{{ logs }}</pre>
              <n-empty
                v-else
                :description="dict.container.logs.empty"
                class="terminal-empty"
              />
            </div>
          </n-spin>
        </n-tab-pane>

        <n-tab-pane name="exec" :tab="dict.container.exec.title">
          <div class="panel-intro">
            <div class="panel-heading">
              <TerminalSquare :size="18" />{{ dict.container.exec.title }}
            </div>
            <p>{{ dict.container.exec.subtitle }}</p>
          </div>
          <n-alert
            v-if="container?.state !== 'running'"
            type="warning"
            :bordered="false"
            class="exec-alert"
          >
            {{ dict.container.exec.notRunning }}
          </n-alert>
          <TerminalPanel
            v-else-if="container"
            :node-id="nodeId"
            :container-id="container.id"
            :active="visible && activeTab === 'exec'"
          />
        </n-tab-pane>
      </n-tabs>
    </n-drawer-content>
  </n-drawer>
</template>

<style scoped>
.drawer-title {
  display: flex;
  align-items: center;
  gap: 10px;
}
.drawer-title code {
  padding: 3px 7px;
  border-radius: 4px;
  background: var(--sw-surface-soft);
  color: var(--sw-text-secondary);
  font-size: 10px;
  font-weight: 400;
}
.panel-intro {
  margin: 7px 0 20px;
}
.panel-heading {
  display: flex;
  align-items: center;
  gap: 9px;
  color: var(--sw-text);
  font-size: 15px;
  font-weight: 600;
}
.panel-heading :deep(svg) {
  color: var(--sw-primary);
}
.panel-intro p {
  margin: 7px 0 0 27px;
  color: var(--sw-muted);
  font-size: 11px;
}
.log-toolbar {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px;
  margin-bottom: 12px;
}
.tail-select {
  width: 82px;
}
.follow-toggle {
  display: flex;
  align-items: center;
  gap: 7px;
  margin-right: auto;
  color: var(--sw-text-secondary);
  font-size: 12px;
}
.terminal {
  position: relative;
  overflow: hidden;
  border: 1px solid var(--sw-border);
  border-radius: 8px;
  background: var(--sw-terminal-bg);
  box-shadow: var(--sw-shadow);
}
.terminal pre {
  min-height: 100%;
  margin: 0;
  padding: 16px;
  overflow: auto;
  color: var(--sw-terminal-text);
  font-family: "Cascadia Code", Consolas, monospace;
  font-size: 11px;
  line-height: 1.75;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}
.log-terminal,
.log-terminal pre {
  height: min(570px, calc(100vh - 265px));
}
.terminal-empty {
  position: absolute;
  inset: 0;
  display: flex;
  justify-content: center;
}
.terminal-empty :deep(.n-empty__description) {
  color: var(--sw-muted);
}
.exec-alert {
  margin-bottom: 16px;
}
</style>
