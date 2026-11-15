<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { NButton, NButtonGroup, NEmpty, NIcon, NPopconfirm, NSpin, NTag } from "naive-ui";
import { connectSuccess, departSuccess, steerSuccess } from "../../utils/message";
import {
  Check,
  Grid3X3,
  Link,
  List,
  Pencil,
  Plus,
  RefreshCw,
  Ship,
  ShieldCheck,
  Trash2,
} from "lucide-vue-next";
import NodeFormModal from "../../components/NodeFormModal.vue";
import { dict } from "../../dict";
import { useFleetStore } from "../../store/fleet";
import type { ShipNode } from "../../types";

const fleet = useFleetStore();
const modalVisible = ref(false);
const editingNode = ref<ShipNode | null>(null);
const nodes = computed(() => fleet.state.nodes);
const viewMode = ref<"card" | "table">(localStorage.getItem("shipwiz.fleet.view") === "table" ? "table" : "card");
watch(viewMode, (value) => localStorage.setItem("shipwiz.fleet.view", value));

onMounted(async () => {
  await fleet.load();
  await fleet.loadOverview();
});

function openCreate() {
  editingNode.value = null;
  modalVisible.value = true;
}
function openEdit(node: ShipNode) {
  editingNode.value = node;
  modalVisible.value = true;
}
async function refresh() {
  await fleet.load();
  await fleet.loadOverview();
}
function onSaved(mode: "create" | "edit") {
  steerSuccess(mode === "create" ? dict.fleet.created : dict.fleet.updated);
  void fleet.loadOverview();
}
async function remove(id: string) {
  if (await fleet.remove(id)) {
    departSuccess(dict.fleet.removed);
    await fleet.loadOverview();
  }
}
async function setCurrent(id: string) {
  if (await fleet.setCurrent(id)) steerSuccess(dict.fleet.switched);
}
async function test(id: string) {
  if (await fleet.test(id)) connectSuccess(dict.fleet.connected);
  await fleet.loadOverview();
}
function connectionKind(node: ShipNode) {
  return node.connection ?? (node.type === "local" ? "local" : "tls");
}
function connectionLabel(node: ShipNode) {
  const kind = connectionKind(node);
  return kind === "local" ? dict.common.local : kind === "ssh" ? dict.fleet.sshConnection : dict.fleet.tlsConnection;
}
function connectionAddress(node: ShipNode) {
  if (connectionKind(node) === "local") return dict.fleet.localConnection;
  return `${connectionKind(node) === "ssh" && node.username ? `${node.username}@` : ""}${node.host}:${node.port}`;
}
function formatLastSuccess(value: number) {
  return new Intl.DateTimeFormat("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  }).format(new Date(value));
}
</script>

<template>
  <div class="page fleet-page">
    <header class="page-header">
      <div class="page-heading">
        <span class="eyebrow">{{ dict.fleet.eyebrow }}</span>
        <h1>{{ dict.fleet.title }}</h1>
        <p>{{ dict.fleet.description }}</p>
      </div>
      <div class="header-actions">
        <n-button type="primary" size="large" @click="openCreate">
          <template #icon><n-icon><Plus /></n-icon></template>
          {{ dict.common.add }}
        </n-button>
      </div>
    </header>

    <div class="stats-grid stats-grid--five">
      <div class="stat-card">
        <span>{{ dict.fleet.total }}</span><strong>{{ nodes.length }}</strong>
      </div>
      <div class="stat-card accent">
        <span>{{ dict.fleet.online }}</span><strong>{{ fleet.onlineCount }}</strong>
      </div>
      <div class="stat-card">
        <span>{{ dict.fleet.totalContainers }}</span>
        <strong>{{ fleet.overviewLoading ? "…" : fleet.overview.containers }}</strong>
      </div>
      <div class="stat-card">
        <span>{{ dict.fleet.totalImages }}</span>
        <strong>{{ fleet.overviewLoading ? "…" : fleet.overview.images }}</strong>
      </div>
      <div class="stat-card accent">
        <span>{{ dict.fleet.runningServices }}</span>
        <strong>{{ fleet.overviewLoading ? "…" : fleet.overview.running }}</strong>
      </div>
    </div>
    <p v-if="fleet.overview.unavailable" class="overview-warning">
      {{ fleet.overview.unavailable }} {{ dict.fleet.overviewFailed }}
    </p>

    <n-spin :show="fleet.loading">
      <div v-if="nodes.length" class="fleet-list">
        <div class="list-toolbar">
          <span>{{ dict.nav.fleet }}</span>
          <div class="toolbar-actions">
            <n-button-group class="view-switch">
              <n-button size="small" :type="viewMode === 'card' ? 'primary' : 'default'" :ghost="viewMode === 'card'" aria-label="卡片视图" @click="viewMode = 'card'"><template #icon><n-icon :component="Grid3X3" /></template></n-button>
              <n-button size="small" :type="viewMode === 'table' ? 'primary' : 'default'" :ghost="viewMode === 'table'" aria-label="表格视图" @click="viewMode = 'table'"><template #icon><n-icon :component="List" /></template></n-button>
            </n-button-group>
            <n-button quaternary size="small" :loading="fleet.loading || fleet.overviewLoading" @click="refresh">
              <template #icon><n-icon><RefreshCw /></n-icon></template>
              {{ dict.common.refresh }}
            </n-button>
          </div>
        </div>
        <div v-if="viewMode === 'card'" class="ship-grid">
          <article
            v-for="node in nodes"
            :key="node.id"
            class="ship-card"
            :class="{ 'is-current': fleet.state.currentNodeId === node.id }"
          >
            <div class="ship-card-top">
              <div class="ship-avatar"><Ship :size="24" :stroke-width="1.5" /></div>
              <div class="ship-state" :class="node.status">
                <span class="status-light" :class="node.status" />
                {{ node.status === 'online' ? dict.common.online : dict.common.offline }}
              </div>
            </div>
            <div class="ship-identity">
              <div class="ship-name-row">
                <h2>{{ node.name }}</h2>
                <n-tag v-if="fleet.state.currentNodeId === node.id" size="small" type="success" :bordered="false">
                  {{ dict.common.current }}
                </n-tag>
              </div>
              <p>{{ node.alias || dict.fleet.unnamed }}</p>
            </div>
            <div class="endpoint">
              <Link :size="14" />
              <span>{{ connectionAddress(node) }}</span>
            </div>
            <div class="ship-tags">
              <n-tag size="small" :bordered="false">{{ connectionLabel(node) }}</n-tag>
              <n-tag v-if="node.type === 'local'" size="small" :bordered="false">{{ dict.fleet.localTag }}</n-tag>
              <n-tag v-else size="small" :bordered="false" type="success">
                <template #icon><n-icon :component="ShieldCheck" /></template>
                {{ connectionKind(node) === 'ssh' ? 'SSH 隧道' : dict.fleet.secureTag }}
              </n-tag>
            </div>
            <p v-if="connectionKind(node) === 'ssh'" class="connection-meta">{{ node.remoteSocket || '/var/run/docker.sock' }}</p>
            <p v-if="node.lastSuccessTime" class="connection-meta">{{ dict.fleet.lastSuccess }} {{ formatLastSuccess(node.lastSuccessTime) }}</p>
            <div class="ship-actions">
              <n-button
                size="small"
                :loading="fleet.testingId === node.id"
                @click="test(node.id)"
              >
                {{ fleet.testingId === node.id ? dict.common.testing : dict.common.test }}
              </n-button>
              <n-button
                size="small"
                :type="fleet.state.currentNodeId === node.id ? 'primary' : 'default'"
                :ghost="fleet.state.currentNodeId === node.id"
                :disabled="fleet.state.currentNodeId === node.id"
                @click="setCurrent(node.id)"
              >
                <template v-if="fleet.state.currentNodeId === node.id" #icon><n-icon><Check /></n-icon></template>
                {{ fleet.state.currentNodeId === node.id ? dict.common.current : dict.common.switchTo }}
              </n-button>
              <div class="icon-actions">
                <n-button quaternary circle size="small" :aria-label="dict.common.edit" @click="openEdit(node)">
                  <template #icon><n-icon><Pencil /></n-icon></template>
                </n-button>
                <n-popconfirm
                  :positive-text="dict.common.delete"
                  :negative-text="dict.common.cancel"
                  @positive-click="remove(node.id)"
                >
                  <template #trigger>
                    <n-button quaternary circle size="small" :aria-label="dict.common.delete">
                      <template #icon><n-icon><Trash2 /></n-icon></template>
                    </n-button>
                  </template>
                  <strong>{{ dict.fleet.removeTitle }}</strong>
                  <p class="confirm-copy">{{ dict.fleet.removeDescription }}</p>
                </n-popconfirm>
              </div>
            </div>
          </article>
        </div>
        <div v-else class="ship-table-wrap">
          <table class="ship-table">
            <thead><tr><th>舰船</th><th>连接通道</th><th>状态</th><th>地址</th><th>操作</th></tr></thead>
            <tbody>
              <tr v-for="node in nodes" :key="node.id" :class="{ 'is-current': fleet.state.currentNodeId === node.id }">
                <td><div class="table-ship-name"><Ship :size="16" /><div><strong>{{ node.name }}</strong><span>{{ node.alias || dict.fleet.unnamed }}</span></div><n-tag v-if="fleet.state.currentNodeId === node.id" size="small" type="success" :bordered="false">{{ dict.common.current }}</n-tag></div></td>
                <td>{{ connectionLabel(node) }}</td>
                <td><span class="table-status"><span class="status-light" :class="node.status" />{{ node.status === 'online' ? dict.common.online : dict.common.offline }}</span></td>
                <td class="table-address">
                  <span>{{ connectionAddress(node) }}</span>
                  <small v-if="connectionKind(node) === 'ssh'">{{ node.remoteSocket || '/var/run/docker.sock' }}</small>
                  <small v-if="node.lastSuccessTime">{{ dict.fleet.lastSuccess }} {{ formatLastSuccess(node.lastSuccessTime) }}</small>
                </td>
                <td><div class="table-actions">
                  <n-button size="small" quaternary :loading="fleet.testingId === node.id" @click="test(node.id)">{{ dict.common.test }}</n-button>
                  <n-button size="small" quaternary :disabled="fleet.state.currentNodeId === node.id" @click="setCurrent(node.id)">{{ fleet.state.currentNodeId === node.id ? dict.common.current : dict.common.switchTo }}</n-button>
                  <n-button size="small" quaternary circle :aria-label="dict.common.edit" @click="openEdit(node)"><template #icon><n-icon :component="Pencil" /></template></n-button>
                  <n-popconfirm :positive-text="dict.common.delete" :negative-text="dict.common.cancel" @positive-click="remove(node.id)"><template #trigger><n-button size="small" quaternary circle :aria-label="dict.common.delete"><template #icon><n-icon :component="Trash2" /></template></n-button></template><strong>{{ dict.fleet.removeTitle }}</strong><p class="confirm-copy">{{ dict.fleet.removeDescription }}</p></n-popconfirm>
                </div></td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>
      <div v-else-if="!fleet.loading && fleet.failed && fleet.state.nodes.length === 0" class="empty-fleet">
        <n-empty description="舰队信息加载失败，请检查应用配置后重试。">
          <template #extra><n-button type="primary" @click="refresh">{{ dict.common.retry }}</n-button></template>
        </n-empty>
      </div>
      <div v-else-if="!fleet.loading && !fleet.failed && fleet.state.nodes.length === 0" class="empty-fleet">
        <n-empty :description="dict.fleet.emptyDescription">
          <template #icon><Ship :size="52" :stroke-width="1" /></template>
          <template #extra>
            <n-button type="primary" @click="openCreate">{{ dict.common.add }}</n-button>
          </template>
        </n-empty>
        <h3>{{ dict.fleet.emptyTitle }}</h3>
      </div>
    </n-spin>

    <NodeFormModal v-model:show="modalVisible" :node="editingNode" @saved="onSaved" />
  </div>
</template>

<style scoped>
.overview-warning { margin: 0 0 22px; color: var(--sw-warning); font-size: 11px; }
.stat-card { position: relative; padding: 16px; overflow: hidden; border: 1px solid var(--sw-border); border-radius: 8px; background: var(--sw-surface); }
.stat-card::after { content: ""; position: absolute; right: -20px; bottom: -38px; width: 90px; height: 90px; border: 1px solid var(--sw-border); border-radius: 50%; }
.stat-card span { display: block; color: var(--sw-text-secondary); font-size: 12px; }
.stat-card strong { display: inline-block; margin-top: 7px; color: var(--sw-text); font-family: Georgia, serif; font-size: 31px; font-weight: 500; line-height: 1; }
.stat-card small { margin-left: 10px; color: var(--sw-muted); font-size: 9px; letter-spacing: 1.5px; }
.stat-card.accent { border-color: var(--sw-primary); background: var(--sw-primary-soft); }
.stat-card.accent strong { color: var(--sw-primary); }
.list-toolbar > span { color: var(--sw-text-secondary); font-size: 12px; font-weight: 600; }
.ship-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(310px, 1fr)); gap: 16px; }
.ship-card { position: relative; padding: 16px; border: 1px solid var(--sw-border); border-radius: 8px; background: var(--sw-surface); transition: border-color 300ms, box-shadow 300ms, transform 300ms; }
.ship-card:hover { transform: translateY(-2px); border-color: var(--sw-border-strong); box-shadow: var(--sw-shadow); }
.ship-card.is-current { border-color: var(--sw-primary); box-shadow: inset 0 3px 0 var(--sw-primary); }
.ship-card-top { display: flex; align-items: flex-start; justify-content: space-between; }
.ship-avatar { display: grid; place-items: center; width: 44px; height: 44px; color: var(--sw-primary); border: 1px solid var(--sw-border); border-radius: 8px; background: var(--sw-primary-soft); }
.ship-state { display: flex; align-items: center; gap: 6px; padding-top: 3px; color: var(--sw-muted); font-size: 11px; }
.ship-state.online { color: var(--sw-success); }
.ship-identity { margin: 16px 0; }
.ship-name-row { display: flex; align-items: center; gap: 9px; }
.ship-name-row h2 { margin: 0; overflow: hidden; color: var(--sw-text); font-size: 17px; font-weight: 600; text-overflow: ellipsis; white-space: nowrap; }
.ship-identity p { margin: 6px 0 0; overflow: hidden; color: var(--sw-muted); font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
.endpoint { display: flex; align-items: center; gap: 7px; min-height: 36px; padding: 9px 11px; color: var(--sw-text-secondary); border: 1px solid var(--sw-border); border-radius: 8px; background: var(--sw-surface-soft); font-family: Consolas, monospace; font-size: 11px; }
.endpoint span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.ship-tags { display: flex; gap: 6px; min-height: 24px; margin: 12px 0 18px; }
.connection-meta { margin: -10px 0 12px; overflow: hidden; color: var(--sw-muted); font-size: 10px; text-overflow: ellipsis; white-space: nowrap; }
.connection-meta + .connection-meta { margin-top: -6px; }
.table-address small { display: block; margin-top: 4px; color: var(--sw-muted); font-size: 10px; }
.ship-actions { display: flex; align-items: center; gap: 8px; padding-top: 15px; border-top: 1px solid var(--sw-border); }
.icon-actions { display: flex; margin-left: auto; }
.confirm-copy { max-width: 240px; margin: 7px 0 0; color: var(--sw-text-secondary); font-size: 12px; font-weight: 400; }
.empty-fleet { display: flex; min-height: 300px; flex-direction: column; align-items: center; justify-content: center; border: 1px dashed var(--sw-border-strong); border-radius: 8px; background: var(--sw-surface); }
.empty-fleet h3 { order: -1; margin: 0 0 4px; color: var(--sw-text); font-size: 16px; }
@media (max-width: 1150px) {
  .ship-grid { grid-template-columns: 1fr; }
}
</style>
