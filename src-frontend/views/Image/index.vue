<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import {
  NButton,
  NEmpty,
  NIcon,
  NInput,
  NPopconfirm,
  NSpin,
  NTable,
  NTabPane,
  NTabs,
  NTag,
  NTooltip,
} from "naive-ui";
import { open, save } from "@tauri-apps/plugin-dialog";
import {
  Archive,
  Box,
  Boxes,
  Download,
  FileInput,
  FileOutput,
  PackageOpen,
  Plus,
  RefreshCw,
  Search,
  Ship,
  Star,
  Trash2,
} from "lucide-vue-next";
import PullImageModal from "../../components/PullImageModal.vue";
import RegistryManager from "../../components/RegistryManager.vue";
import { dict } from "../../dict";
import { useFleetStore } from "../../store/fleet";
import { useImageStore } from "../../store/image";
import type { ImageInfo } from "../../types";

const fleet = useFleetStore();
const images = useImageStore();
const ready = ref(false);
const activeTab = ref("local");
const localSearch = ref("");
const onlineTerm = ref("");
const searched = ref(false);
const pullVisible = ref(false);
const pullReference = ref("");
const currentNodeId = computed(() => fleet.state.currentNodeId);
const currentNode = computed(() => fleet.currentNode);
const unusedCount = computed(() => Math.max(images.items.length - images.inUseCount, 0));
const filteredImages = computed(() => {
  const keyword = localSearch.value.trim().toLowerCase();
  if (!keyword) return images.items;
  return images.items.filter((image) =>
    [image.id, image.shortId, ...image.repoTags, ...image.repoDigests]
      .some((value) => value.toLowerCase().includes(keyword)),
  );
});
const emptyDescription = computed(() => {
  if (images.failed) return dict.image.loadFailed;
  if (localSearch.value) return dict.image.emptyFiltered;
  return dict.image.emptyDescription;
});

onMounted(async () => {
  const tasks: Promise<unknown>[] = [images.loadRegistries()];
  if (!fleet.state.nodes.length) tasks.push(fleet.load());
  await Promise.all(tasks);
  ready.value = true;
  if (currentNodeId.value) await images.load(currentNodeId.value);
});
watch(currentNodeId, async (nodeId, previousNodeId) => {
  if (!ready.value || !nodeId || nodeId === previousNodeId) return;
  images.searchResults = [];
  searched.value = false;
  await images.load(nodeId);
});

function formatBytes(bytes: number) {
  if (!bytes) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const index = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  return `${(bytes / 1024 ** index).toFixed(index ? 1 : 0)} ${units[index]}`;
}
function formatDate(timestamp: number) {
  if (!timestamp) return "—";
  return new Intl.DateTimeFormat("zh-CN", { year: "numeric", month: "2-digit", day: "2-digit" })
    .format(new Date(timestamp * 1000));
}
function displayTags(image: ImageInfo) {
  const tags = image.repoTags.filter((tag) => tag !== "<none>:<none>");
  if (tags.length) return tags;
  const digests = image.repoDigests.filter((digest) => !digest.includes("<none>"));
  return digests.length ? digests : [dict.image.untagged];
}
function exportReference(image: ImageInfo) {
  return image.repoTags.find((tag) => tag !== "<none>:<none>") ?? image.id;
}
function openPull(reference = "") {
  pullReference.value = reference;
  pullVisible.value = true;
}
async function refresh() {
  if (currentNodeId.value) await images.load(currentNodeId.value);
}
async function searchOnline() {
  if (!currentNodeId.value || !onlineTerm.value.trim()) return;
  searched.value = true;
  await images.search(currentNodeId.value, onlineTerm.value);
}
async function importArchive() {
  if (!currentNodeId.value) return;
  const source = await open({
    title: dict.image.fileDialog.importTitle,
    multiple: false,
    directory: false,
    filters: [{ name: dict.image.fileDialog.archive, extensions: ["tar"] }],
  });
  if (source) await images.importImage(currentNodeId.value, source);
}
async function exportArchive(image: ImageInfo) {
  if (!currentNodeId.value) return;
  const reference = exportReference(image);
  const filename = `${reference.replace(/[^a-zA-Z0-9._-]+/g, "-")}.tar`;
  const destination = await save({
    title: dict.image.fileDialog.exportTitle,
    defaultPath: filename,
    filters: [{ name: dict.image.fileDialog.archive, extensions: ["tar"] }],
  });
  if (destination) await images.exportImage(currentNodeId.value, reference, destination);
}
</script>

<template>
  <section class="image-page page page-enter">
    <header class="page-header">
      <div>
        <span class="eyebrow">{{ dict.image.eyebrow }}</span>
        <h1>{{ dict.image.title }}</h1>
        <p>{{ dict.image.description }}</p>
      </div>
      <div class="header-actions">
        <n-button size="large" :disabled="!currentNode" :loading="images.actionKey === 'import'" @click="importArchive">
          <template #icon><n-icon><FileInput /></n-icon></template>
          {{ dict.image.import }}
        </n-button>
        <n-button type="primary" size="large" :disabled="!currentNode" @click="openPull()">
          <template #icon><n-icon><Plus /></n-icon></template>
          {{ dict.image.pull }}
        </n-button>
      </div>
    </header>

    <div class="current-ship-strip">
      <div class="ship-emblem"><Ship :size="21" /></div>
      <div class="ship-copy">
        <span>{{ dict.image.currentShip }}</span>
        <strong>{{ currentNode?.name ?? "—" }}</strong>
      </div>
      <code>{{ currentNode?.type === "local" ? dict.fleet.local : `${currentNode?.host}:${currentNode?.port}` }}</code>
      <span class="connection-state" :class="currentNode?.status">
        <i />{{ currentNode?.status === "online" ? dict.fleet.online : dict.common.offline }}
      </span>
      <n-button quaternary circle :loading="images.loading" :aria-label="dict.common.refresh" @click="refresh">
        <template #icon><n-icon><RefreshCw /></n-icon></template>
      </n-button>
    </div>

    <div class="image-stats">
      <div class="metric-card">
        <span class="metric-icon teal"><Boxes :size="18" /></span>
        <div><span>{{ dict.image.total }}</span><strong>{{ images.items.length.toString().padStart(2, "0") }}</strong></div>
      </div>
      <div class="metric-card">
        <span class="metric-icon blue"><Archive :size="18" /></span>
        <div><span>{{ dict.image.totalSize }}</span><strong class="size-value">{{ formatBytes(images.totalSize) }}</strong></div>
      </div>
      <div class="metric-card">
        <span class="metric-icon green"><PackageOpen :size="18" /></span>
        <div><span>{{ dict.image.inUse }}</span><strong>{{ images.inUseCount.toString().padStart(2, "0") }}</strong></div>
      </div>
      <div class="metric-card">
        <span class="metric-icon slate"><Box :size="18" /></span>
        <div><span>{{ dict.image.unused }}</span><strong>{{ unusedCount.toString().padStart(2, "0") }}</strong></div>
      </div>
    </div>

    <div class="image-panel">
      <n-tabs v-model:value="activeTab" type="line" animated class="image-tabs">
        <n-tab-pane name="local" :tab="dict.image.tabs.local">
          <div class="list-toolbar">
            <n-input v-model:value="localSearch" class="local-search" :placeholder="dict.image.searchLocal" clearable>
              <template #prefix><n-icon><Search /></n-icon></template>
            </n-input>
            <n-button secondary :loading="images.loading" @click="refresh">
              <template #icon><n-icon><RefreshCw /></n-icon></template>
              {{ dict.image.refresh }}
            </n-button>
          </div>
          <n-spin :show="images.loading">
            <div v-if="filteredImages.length" class="table-scroll">
              <n-table :single-line="false" class="image-table">
                <thead><tr>
                  <th>{{ dict.image.table.reference }}</th>
                  <th>{{ dict.image.table.id }}</th>
                  <th>{{ dict.image.table.size }}</th>
                  <th>{{ dict.image.table.created }}</th>
                  <th>{{ dict.image.table.usage }}</th>
                  <th>{{ dict.image.table.actions }}</th>
                </tr></thead>
                <tbody>
                  <tr v-for="image in filteredImages" :key="image.id">
                    <td>
                      <div class="image-identity">
                        <span class="image-cube"><Boxes :size="17" /></span>
                        <div>
                          <strong v-for="tag in displayTags(image).slice(0, 2)" :key="tag" :title="tag">{{ tag }}</strong>
                          <span v-if="displayTags(image).length > 2">+{{ displayTags(image).length - 2 }}</span>
                        </div>
                      </div>
                    </td>
                    <td><code class="image-id">{{ image.shortId }}</code></td>
                    <td><span class="numeric-value">{{ formatBytes(image.size) }}</span></td>
                    <td><span class="date-value">{{ formatDate(image.created) }}</span></td>
                    <td>
                      <n-tag size="small" :bordered="false" :type="image.containers > 0 ? 'success' : 'default'">
                        {{ image.containers > 0 ? `${image.containers} ${dict.image.references}` : dict.image.noReferences }}
                      </n-tag>
                    </td>
                    <td>
                      <div class="row-actions">
                        <n-tooltip>
                          <template #trigger>
                            <n-button quaternary circle size="small" :loading="images.actionKey === `export:${exportReference(image)}`" @click="exportArchive(image)">
                              <template #icon><n-icon><FileOutput /></n-icon></template>
                            </n-button>
                          </template>
                          {{ dict.image.export }}
                        </n-tooltip>
                        <n-popconfirm
                          :positive-text="dict.common.delete"
                          :negative-text="dict.common.cancel"
                          @positive-click="currentNodeId && images.remove(currentNodeId, image)"
                        >
                          <template #trigger>
                            <n-button quaternary circle size="small" type="error" :loading="images.actionKey === `remove:${image.id}`">
                              <template #icon><n-icon><Trash2 /></n-icon></template>
                            </n-button>
                          </template>
                          <strong>{{ dict.image.removeTitle }}</strong>
                          <p class="remove-hint">{{ dict.image.removeDescription }}</p>
                        </n-popconfirm>
                      </div>
                    </td>
                  </tr>
                </tbody>
              </n-table>
            </div>
            <n-empty v-else :description="emptyDescription" class="image-empty">
              <template #icon><Boxes :size="45" stroke-width="1.2" /></template>
              <template v-if="!images.failed && !localSearch" #extra>
                <n-button type="primary" @click="openPull()">{{ dict.image.pull }}</n-button>
              </template>
            </n-empty>
          </n-spin>
        </n-tab-pane>

        <n-tab-pane name="online" :tab="dict.image.tabs.online">
          <div class="online-search-box">
            <div class="search-emblem"><Download :size="23" /></div>
            <n-input
              v-model:value="onlineTerm"
              size="large"
              :placeholder="dict.image.searchPlaceholder"
              clearable
              @keyup.enter="searchOnline"
            >
              <template #prefix><n-icon><Search /></n-icon></template>
            </n-input>
            <n-button type="primary" size="large" :loading="images.searching" @click="searchOnline">
              {{ images.searching ? dict.image.searching : dict.image.search }}
            </n-button>
          </div>
          <n-spin :show="images.searching">
            <div v-if="images.searchResults.length" class="search-results">
              <article v-for="result in images.searchResults" :key="result.name" class="search-result-card">
                <div class="result-icon"><PackageOpen :size="21" /></div>
                <div class="result-copy">
                  <div class="result-title">
                    <h3>{{ result.name }}</h3>
                    <n-tag size="small" :bordered="false" :type="result.isOfficial ? 'success' : 'default'">
                      {{ result.isOfficial ? dict.image.official : dict.image.community }}
                    </n-tag>
                  </div>
                  <p>{{ result.description || dict.image.noDescription }}</p>
                  <span class="star-count"><Star :size="12" />{{ result.starCount }} {{ dict.image.stars }}</span>
                </div>
                <n-button secondary type="primary" @click="openPull(result.name)">{{ dict.image.pull }}</n-button>
              </article>
            </div>
            <n-empty v-else :description="searched ? dict.image.noResults : dict.image.searchHint" class="image-empty">
              <template #icon><Search :size="42" stroke-width="1.2" /></template>
            </n-empty>
          </n-spin>
        </n-tab-pane>

        <n-tab-pane name="registries" :tab="dict.image.tabs.registries">
          <RegistryManager />
        </n-tab-pane>
      </n-tabs>
    </div>

    <PullImageModal
      v-model:show="pullVisible"
      :node-id="currentNodeId ?? ''"
      :reference="pullReference"
      @pulled="activeTab = 'local'"
    />
  </section>
</template>

<style scoped>
.header-actions { display: flex; gap: 10px; margin-top: 10px; }
.current-ship-strip { display: flex; align-items: center; gap: 13px; min-height: 64px; padding: 10px 16px; margin-bottom: 18px; border: 1px solid var(--sw-border); border-radius: 8px; background: var(--sw-surface); }
.ship-emblem { display: grid; place-items: center; width: 38px; height: 38px; color: var(--sw-terminal-text); border-radius: 8px; background: var(--sw-primary); }
.ship-copy { display: flex; flex-direction: column; min-width: 120px; }
.ship-copy span { color: var(--sw-muted); font-size: 9px; }
.ship-copy strong { margin-top: 3px; color: var(--sw-text); font-size: 13px; font-weight: 500; }
.current-ship-strip > code { margin-left: 5px; color: var(--sw-muted); font-size: 10px; }
.connection-state { display: flex; align-items: center; gap: 6px; margin-left: auto; color: var(--sw-muted); font-size: 11px; }
.connection-state i { width: 6px; height: 6px; border-radius: 50%; background: var(--sw-muted); }
.connection-state.online { color: var(--sw-success); }
.connection-state.online i { background: var(--sw-success); box-shadow: 0 0 0 4px var(--sw-success-soft); }
.image-stats { display: grid; grid-template-columns: repeat(4, 1fr); gap: 13px; margin-bottom: 18px; }
.metric-card { display: flex; align-items: center; gap: 14px; min-height: 84px; padding: 16px; border: 1px solid var(--sw-border); border-radius: 8px; background: var(--sw-surface); }
.metric-icon { display: grid; place-items: center; width: 36px; height: 36px; border-radius: 8px; }
.metric-icon.teal { color: var(--sw-primary); background: var(--sw-primary-soft); }
.metric-icon.green { color: var(--sw-success); background: var(--sw-success-soft); }
.metric-icon.slate { color: var(--sw-text-secondary); background: var(--sw-surface-soft); }
.metric-icon.blue { color: var(--sw-primary); background: var(--sw-primary-soft); }
.metric-card > div { display: flex; flex-direction: column; }
.metric-card span:not(.metric-icon) { color: var(--sw-muted); font-size: 10px; }
.metric-card strong { margin-top: 4px; color: var(--sw-text); font-family: "DIN Alternate", Consolas, sans-serif; font-size: 24px; font-weight: 500; line-height: 1; }
.metric-card strong.size-value { font-size: 19px; }
.image-panel { overflow: hidden; border: 1px solid var(--sw-border); border-radius: 8px; background: var(--sw-surface); box-shadow: var(--sw-shadow); }
.image-tabs { padding: 0 16px 16px; }
.image-tabs :deep(.n-tabs-nav) { padding: 0 2px; }
.list-toolbar { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 3px 0 16px; }
.local-search { width: 320px; }
.table-scroll { overflow-x: auto; margin: 0 -16px -16px; }
.image-table { min-width: 920px; font-size: 11px; }
.image-table :deep(th) { height: 40px; color: var(--sw-muted); background: var(--sw-surface-soft); font-size: 10px; font-weight: 400; }
.image-table :deep(td) { height: 72px; color: var(--sw-text-secondary); }
.image-table :deep(th:first-child) { width: 330px; padding-left: 16px; }
.image-table :deep(td:first-child) { padding-left: 16px; }
.image-table :deep(th:nth-child(2)) { width: 120px; }
.image-table :deep(th:nth-child(3)), .image-table :deep(th:nth-child(4)) { width: 110px; }
.image-table :deep(th:nth-child(5)) { width: 100px; }
.image-table :deep(th:last-child) { width: 100px; }
.image-identity { display: flex; align-items: center; gap: 11px; min-width: 0; }
.image-cube { display: grid; place-items: center; flex: 0 0 auto; width: 32px; height: 32px; color: var(--sw-primary); border: 1px solid var(--sw-border); border-radius: 8px; background: var(--sw-primary-soft); }
.image-identity > div { display: flex; flex-direction: column; min-width: 0; }
.image-identity strong { overflow: hidden; max-width: 270px; color: var(--sw-text); font-family: Consolas, monospace; font-size: 10px; font-weight: 500; text-overflow: ellipsis; white-space: nowrap; }
.image-identity strong + strong { margin-top: 4px; color: var(--sw-text-secondary); }
.image-identity span { margin-top: 3px; color: var(--sw-muted); font-size: 9px; }
.image-id { color: var(--sw-text-secondary); font-size: 10px; }
.numeric-value { color: var(--sw-text-secondary); font-family: Consolas, monospace; font-size: 10px; }
.date-value { color: var(--sw-muted); font-size: 10px; }
.row-actions { display: flex; gap: 2px; }
.remove-hint { max-width: 260px; margin: 6px 0 0; line-height: 1.6; }
.image-empty { min-height: 330px; justify-content: center; }
.online-search-box { display: flex; align-items: center; gap: 11px; padding: 16px; margin-bottom: 16px; border: 1px solid var(--sw-border); border-radius: 8px; background: linear-gradient(105deg, var(--sw-primary-soft), var(--sw-surface)); }
.search-emblem { display: grid; place-items: center; flex: 0 0 auto; width: 42px; height: 42px; color: var(--sw-terminal-text); border-radius: 8px; background: var(--sw-primary); }
.search-results { display: grid; gap: 9px; }
.search-result-card { display: flex; align-items: center; gap: 14px; padding: 16px; border: 1px solid var(--sw-border); border-radius: 8px; background: var(--sw-surface); transition: border-color 300ms, transform 300ms; }
.search-result-card:hover { border-color: var(--sw-border-strong); transform: translateY(-1px); }
.result-icon { display: grid; place-items: center; flex: 0 0 auto; width: 38px; height: 38px; color: var(--sw-primary); border-radius: 8px; background: var(--sw-primary-soft); }
.result-copy { flex: 1; min-width: 0; }
.result-title { display: flex; align-items: center; gap: 8px; }
.result-title h3 { overflow: hidden; margin: 0; color: var(--sw-text); font-family: Consolas, monospace; font-size: 12px; font-weight: 500; text-overflow: ellipsis; white-space: nowrap; }
.result-copy p { overflow: hidden; margin: 5px 0; color: var(--sw-text-secondary); font-size: 10px; text-overflow: ellipsis; white-space: nowrap; }
.star-count { display: flex; align-items: center; gap: 4px; color: var(--sw-warning); font-size: 9px; }
@media (max-width: 1100px) {
  .image-stats { grid-template-columns: repeat(2, 1fr); }
  .current-ship-strip > code { display: none; }
  .local-search { width: 240px; }
}
</style>
