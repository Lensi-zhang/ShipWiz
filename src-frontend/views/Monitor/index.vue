<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { NButton, NEmpty, NIcon, NProgress, NSkeleton, NTag } from "naive-ui";
import { Activity, AlertTriangle, Box, Clock3, Cpu, HardDrive, MemoryStick, RefreshCw } from "lucide-vue-next";
import { containerApi } from "../../api/container";
import { imageApi } from "../../api/image";
import { useFleetStore } from "../../store/fleet";
import TermHint from "../../components/TermHint.vue";
import type { ContainerInfo, ImageInfo } from "../../types";

interface Sample { time: string; cpu: number; memory: number }
const fleet = useFleetStore();
const containers = ref<ContainerInfo[]>([]);
const images = ref<ImageInfo[]>([]);
const loading = ref(true);
const failed = ref(false);
const samples = ref<Sample[]>([]);
const lastUpdated = ref("");
let timer: ReturnType<typeof setInterval> | undefined;
let sequence = 0;

const running = computed(() => containers.value.filter((item) => item.state === "running"));
const averageCpu = computed(() => running.value.length ? running.value.reduce((sum, item) => sum + item.cpuPercent, 0) / running.value.length : 0);
const memoryUsed = computed(() => running.value.reduce((sum, item) => sum + item.memoryUsage, 0));
const alerts = computed(() => containers.value.filter((item) => ["dead", "restarting"].includes(item.state) || item.cpuPercent > 90));
const ranked = computed(() => [...containers.value].sort((a, b) => Math.max(b.cpuPercent, b.memoryUsage / 20_000_000) - Math.max(a.cpuPercent, a.memoryUsage / 20_000_000)).slice(0, 6));
const imageBytes = computed(() => images.value.reduce((sum, item) => sum + item.size, 0));
const referencedBytes = computed(() => images.value.filter((item) => item.containers > 0).reduce((sum, item) => sum + item.size, 0));
const referencedRatio = computed(() => imageBytes.value ? Math.round(referencedBytes.value / imageBytes.value * 100) : 0);
const cpuPoints = computed(() => chartPoints(samples.value.map((item) => item.cpu)));
const memoryPoints = computed(() => chartPoints(samples.value.map((item) => item.memory)));

function chartPoints(values: number[]) {
  if (!values.length) return "0,100 400,100";
  const max = Math.max(...values, 1);
  return values.map((value, index) => `${values.length === 1 ? 400 : index / (values.length - 1) * 400},${96 - value / max * 82}`).join(" ");
}
function formatBytes(bytes: number) {
  if (!bytes) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const index = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  return `${(bytes / 1024 ** index).toFixed(index > 2 ? 1 : 0)} ${units[index]}`;
}
async function load() {
  const nodeId = fleet.state.currentNodeId;
  const current = ++sequence;
  if (!nodeId) {
    loading.value = false;
    containers.value = [];
    images.value = [];
    samples.value = [];
    lastUpdated.value = "";
    return;
  }
  failed.value = false;
  try {
    const [containerList, imageList] = await Promise.all([containerApi.list(nodeId), imageApi.list(nodeId)]);
    if (current !== sequence) return;
    containers.value = containerList;
    images.value = imageList;
    const now = new Date();
    lastUpdated.value = now.toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit", second: "2-digit" });
    samples.value = [...samples.value.slice(-11), {
      time: lastUpdated.value,
      cpu: averageCpu.value,
      memory: memoryUsed.value / 1024 / 1024,
    }];
  } catch {
    if (current === sequence) {
      failed.value = true;
      containers.value = [];
      images.value = [];
      lastUpdated.value = "";
    }
  } finally {
    if (current === sequence) loading.value = false;
  }
}
function restartTimer() {
  if (timer) clearInterval(timer);
  timer = setInterval(load, 5000);
}
onMounted(() => { void load(); restartTimer(); });
watch(() => fleet.state.currentNodeId, () => {
  sequence += 1;
  containers.value = [];
  images.value = [];
  samples.value = [];
  lastUpdated.value = "";
  failed.value = false;
  loading.value = true;
  void load();
});
onBeforeUnmount(() => { sequence += 1; if (timer) clearInterval(timer); });
</script>

<template>
  <div class="page monitor-page">
    <header class="page-header">
      <div class="page-heading"><span class="eyebrow">LIVE ENGINE TELEMETRY</span><h1>动力监控台</h1><p>观察当前舰船的实时负载、储物占用与异常信号。</p></div>
      <div class="header-actions"><n-button secondary :loading="loading" @click="load"><template #icon><n-icon :component="RefreshCw" /></template>刷新数据</n-button></div>
    </header>
    <div class="current-ship-bar"><span class="status-light" :class="fleet.currentNode?.status || 'offline'" /><span class="ship-label">当前舰船</span><strong>{{ fleet.currentNode?.name || "尚未选择" }}</strong><span v-if="lastUpdated" class="update-time"><Clock3 :size="12" />{{ lastUpdated }} 更新 · 每 5 秒采样</span></div>

    <div v-if="failed" class="monitor-error" role="alert"><AlertTriangle :size="18" /><div><strong>暂时无法取得动力数据</strong><p>请检查舰船连接；监控不可用不会被记作零负载。</p></div><n-button size="small" @click="load">重新连接</n-button></div>
    <template v-else>
      <div class="monitor-stats stats-grid stats-grid--four">
        <article><span class="metric-icon blue"><Cpu :size="19" /></span><div><span>平均处理器负载</span><strong>{{ averageCpu.toFixed(1) }}<small>%</small></strong><p>{{ running.length }} 个航行服务参与采样</p></div></article>
        <article><span class="metric-icon green"><MemoryStick :size="19" /></span><div><span>运行内存合计</span><strong>{{ formatBytes(memoryUsed) }}</strong><p>仅统计航行中的集装箱</p></div></article>
        <article><span class="metric-icon blue"><Activity :size="19" /></span><div><span>服务运行状态</span><strong>{{ running.length }}<small> / {{ containers.length }}</small></strong><p>航行中 / 全部集装箱</p></div></article>
        <article :class="{ warning: alerts.length }"><span class="metric-icon orange"><AlertTriangle :size="19" /></span><div><span>需要注意</span><strong>{{ alerts.length }}<small> 项</small></strong><p>{{ alerts.length ? "高负载或动力状态异常" : "当前未发现异常信号" }}</p></div></article>
      </div>

      <div class="monitor-grid">
        <section class="telemetry-panel trend-panel">
          <div class="panel-title"><div><h2>负载曲线</h2><p>保留最近 12 次采样，仅用于本次会话观察</p></div><div class="legend"><span class="cpu">处理器</span><span class="memory">内存 MB</span></div></div>
          <div v-if="loading && !samples.length" class="chart-loading"><n-skeleton height="190px" :sharp="false" /></div>
          <div v-else-if="samples.length" class="line-chart">
            <svg viewBox="0 0 400 110" preserveAspectRatio="none" role="img" aria-label="处理器与内存趋势"><path v-for="y in [14,41,68,96]" :key="y" :d="`M0 ${y} H400`" class="grid-line" /><polyline :points="cpuPoints" class="cpu-line" /><polyline :points="memoryPoints" class="memory-line" /></svg>
            <div class="chart-axis"><span>{{ samples[0]?.time }}</span><span>最近采样 {{ lastUpdated }}</span></div>
          </div>
          <n-empty v-else description="等待首次动力采样" />
        </section>

        <section class="telemetry-panel storage-panel">
          <div class="panel-title"><div><h2>储物资源</h2><p>本地图纸总量与被引用比例</p></div><HardDrive :size="18" /></div>
          <div class="storage-ring" :style="{ '--ratio': `${referencedRatio * 3.6}deg` }"><div><strong>{{ referencedRatio }}%</strong><span>图纸被引用</span></div></div>
          <div class="storage-numbers"><div><span>图纸总占用</span><strong>{{ formatBytes(imageBytes) }}</strong></div><div><span>使用中图纸</span><strong>{{ formatBytes(referencedBytes) }}</strong></div></div>
        </section>
      </div>

      <section class="telemetry-panel ranking-panel">
        <div class="panel-title"><div><h2><TermHint term="资源排行" explanation="按处理器与内存的相对占用排序，帮助快速找到需要关注的服务。" /></h2><p>最多展示当前负载最高的 6 只集装箱</p></div><Box :size="18" /></div>
        <div v-if="ranked.length" class="ranking-list">
          <div v-for="(item, index) in ranked" :key="item.id" class="ranking-row">
            <span class="rank">{{ String(index + 1).padStart(2, "0") }}</span><div class="rank-name"><strong>{{ item.name }}</strong><span>{{ item.image }}</span></div>
            <n-tag size="small" :type="item.state === 'running' ? 'success' : 'default'" :bordered="false">{{ item.state === "running" ? "航行中" : "已抛锚" }}</n-tag>
            <div class="resource-bar"><span>处理器 {{ item.cpuPercent.toFixed(1) }}%</span><n-progress type="line" :percentage="Math.min(item.cpuPercent, 100)" :show-indicator="false" :height="4" :color="item.cpuPercent > 90 ? '#F59E0B' : '#165DFF'" /></div>
            <span class="memory-value">{{ formatBytes(item.memoryUsage) }}</span>
          </div>
        </div>
        <n-empty v-else description="当前舰船还没有可排行的集装箱" />
      </section>
    </template>
  </div>
</template>

<style scoped>
.update-time { display: inline-flex; align-items: center; gap: 5px; margin-left: auto; color: var(--sw-muted); }
.monitor-error { display: flex; align-items: center; gap: 14px; padding: 20px; color: var(--sw-warning); border: 1px solid var(--sw-warning); border-radius: 8px; background: var(--sw-warning-soft); }
.monitor-error > div { flex: 1; }.monitor-error strong { color: var(--sw-text); }.monitor-error p { margin: 4px 0 0; color: var(--sw-text-secondary); font-size: 11px; }
.monitor-stats article { display: flex; gap: 13px; min-width: 0; padding: 16px; border: 1px solid var(--sw-border); border-radius: 8px; background: var(--sw-surface); }
.monitor-stats article.warning { border-color: var(--sw-warning); }
.metric-icon { display: grid; place-items: center; flex: 0 0 auto; width: 38px; height: 38px; border-radius: 8px; }.metric-icon.blue { color: var(--sw-primary); background: var(--sw-primary-soft); }.metric-icon.green { color: var(--sw-success); background: var(--sw-success-soft); }.metric-icon.orange { color: var(--sw-warning); background: var(--sw-warning-soft); }
.monitor-stats article > div { min-width: 0; }.monitor-stats span { color: var(--sw-text-secondary); font-size: 11px; }.monitor-stats strong { display: block; margin-top: 6px; overflow: hidden; color: var(--sw-text); font-family: "Bahnschrift", sans-serif; font-size: 25px; font-weight: 600; text-overflow: ellipsis; white-space: nowrap; }.monitor-stats strong small { color: var(--sw-muted); font-size: 12px; }.monitor-stats p { margin: 5px 0 0; overflow: hidden; color: var(--sw-muted); font-size: 9px; text-overflow: ellipsis; white-space: nowrap; }
.monitor-grid { display: grid; grid-template-columns: minmax(0, 1.8fr) minmax(260px, .7fr); gap: 16px; margin-bottom: 16px; }
.telemetry-panel { padding: 16px; border: 1px solid var(--sw-border); border-radius: 8px; background: var(--sw-surface); }
.panel-title { display: flex; align-items: flex-start; justify-content: space-between; gap: 12px; margin-bottom: 16px; color: var(--sw-muted); }.panel-title h2 { margin: 0; color: var(--sw-text); font-size: 14px; font-weight: 600; }.panel-title p { margin: 5px 0 0; color: var(--sw-muted); font-size: 10px; }
.legend { display: flex; gap: 13px; font-size: 10px; }.legend span::before { content: ""; display: inline-block; width: 7px; height: 7px; margin-right: 5px; border-radius: 50%; }.legend .cpu::before { background: var(--sw-primary); }.legend .memory::before { background: var(--sw-success); }
.line-chart svg { width: 100%; height: 190px; overflow: visible; }.grid-line { fill: none; stroke: var(--sw-border); stroke-width: .6; }.cpu-line, .memory-line { fill: none; stroke-width: 2; vector-effect: non-scaling-stroke; }.cpu-line { stroke: var(--sw-primary); }.memory-line { stroke: var(--sw-success); }
.chart-axis { display: flex; justify-content: space-between; color: var(--sw-muted); font-size: 9px; }.chart-loading { padding-top: 10px; }
.storage-panel { min-width: 0; }.storage-ring { display: grid; place-items: center; width: 142px; height: 142px; margin: 4px auto 17px; border-radius: 50%; background: conic-gradient(var(--sw-primary) var(--ratio), var(--sw-border) 0); }.storage-ring::before { content: ""; grid-area: 1 / 1; width: 108px; height: 108px; border-radius: 50%; background: var(--sw-surface); }.storage-ring > div { z-index: 1; grid-area: 1 / 1; text-align: center; }.storage-ring strong { display: block; font-family: "Bahnschrift", sans-serif; font-size: 27px; }.storage-ring span { color: var(--sw-muted); font-size: 10px; }
.storage-numbers { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; }.storage-numbers > div { padding: 9px; border-radius: 6px; background: var(--sw-surface-soft); }.storage-numbers span { display: block; color: var(--sw-muted); font-size: 9px; }.storage-numbers strong { display: block; margin-top: 4px; font-size: 12px; }
.ranking-list { display: flex; flex-direction: column; }.ranking-row { display: grid; grid-template-columns: 30px minmax(160px, 1fr) 70px minmax(150px, .7fr) 80px; align-items: center; gap: 14px; min-height: 54px; border-top: 1px solid var(--sw-border); }.rank { color: var(--sw-muted); font-family: "Bahnschrift", sans-serif; font-size: 11px; }.rank-name { min-width: 0; }.rank-name strong, .rank-name span { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }.rank-name strong { font-size: 12px; }.rank-name span { margin-top: 3px; color: var(--sw-muted); font-size: 9px; }.resource-bar > span { display: block; margin-bottom: 5px; color: var(--sw-text-secondary); font-size: 9px; }.memory-value { color: var(--sw-text-secondary); font-size: 11px; text-align: right; }
@media (max-width: 1200px) { .monitor-grid { grid-template-columns: 1fr; }.ranking-row { grid-template-columns: 26px minmax(140px, 1fr) 65px 140px; }.memory-value { display: none; } }
@media (max-width: 900px) { .update-time { width: 100%; margin-left: 0; }.monitor-error { align-items: flex-start; flex-wrap: wrap; } }
</style>
