<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { getVersion } from "@tauri-apps/api/app";
import { NButton, NIcon, NInputNumber, NSelect, NSwitch, NTabPane, NTabs, NTag, createDiscreteApi } from "naive-ui";
import { Anchor, Check, Info, Moon, RefreshCw, ScrollText, Settings2, Sun } from "lucide-vue-next";
import RegistryManager from "../../components/RegistryManager.vue";
import { useAppearanceStore } from "../../store/appearance";
import { useFleetStore } from "../../store/fleet";
import { useImageStore } from "../../store/image";

const fleet = useFleetStore();
const appearance = useAppearanceStore();
const images = useImageStore();
const { message } = createDiscreteApi(["message"]);
const activeTab = ref("engine");
const version = ref("—");
const testing = ref(false);
const engineMessage = ref("");
const nodeOptions = computed(() => fleet.state.nodes.map((node) => ({ label: `${node.name}（${node.type === "local" ? "本机" : "远程"}）`, value: node.id })));

onMounted(async () => {
  await Promise.all([fleet.load(), images.loadRegistries()]);
  try { version.value = await getVersion(); } catch { version.value = "0.1.0"; }
});
async function testEngine() {
  if (!fleet.state.currentNodeId) return;
  testing.value = true;
  engineMessage.value = "";
  const ok = await fleet.test(fleet.state.currentNodeId);
  testing.value = false;
  engineMessage.value = ok ? "连接检测通过，引擎响应正常。" : "连接检测未通过，请前往舰队总览检查通道配置。";
  if (ok) message.success("动力引擎连接正常");
}
async function switchNode(id: string) {
  engineMessage.value = "";
  await fleet.setCurrent(id);
}
</script>

<template>
  <div class="page setting-page">
    <div class="page-heading"><span class="eyebrow">BRIDGE PREFERENCES</span><h1>设置</h1><p>管理动力通道、补给港和舰桥工作方式，所有变更保存在本机。</p></div>
    <div class="settings-layout">
      <aside class="settings-index">
        <span>设置分组</span>
        <button v-for="item in [{ key: 'engine', label: '动力引擎', icon: Anchor }, { key: 'registries', label: '补给港', icon: RefreshCw }, { key: 'appearance', label: '界面', icon: Settings2 }, { key: 'logs', label: '日志巡检', icon: ScrollText }, { key: 'about', label: '关于', icon: Info }]" :key="item.key" :class="{ active: activeTab === item.key }" @click="activeTab = item.key"><n-icon :component="item.icon" :size="16" />{{ item.label }}</button>
      </aside>
      <n-tabs v-model:value="activeTab" type="line" animated class="settings-tabs">
        <n-tab-pane name="engine" tab="动力引擎">
          <section class="setting-section">
            <div class="section-heading"><div><h2>当前动力通道</h2><p>选择舰船并检测引擎连接。远程通道的详细配置在舰队总览维护。</p></div><n-tag :type="fleet.currentNode?.status === 'online' ? 'success' : 'default'" :bordered="false">{{ fleet.currentNode?.status === "online" ? "在线" : "离线" }}</n-tag></div>
            <div class="setting-row"><div><strong>当前操控舰船</strong><p>容器、图纸、编队和监控都以此舰船为目标。</p></div><n-select :value="fleet.state.currentNodeId" :options="nodeOptions" class="engine-select" @update:value="switchNode" /></div>
            <div class="setting-row"><div><strong>连接状态</strong><p>{{ fleet.currentNode?.alias || "未填写舰船备注" }}</p></div><span class="engine-state"><span class="status-light" :class="fleet.currentNode?.status || 'offline'" />{{ fleet.currentNode?.name || "未选择" }}</span></div>
            <p v-if="engineMessage" class="engine-message" :class="{ success: fleet.currentNode?.status === 'online' }">{{ engineMessage }}</p>
            <div class="section-actions"><n-button type="primary" :loading="testing" :disabled="!fleet.state.currentNodeId" @click="testEngine"><template #icon><n-icon :component="Check" /></template>检测当前通道</n-button><router-link to="/fleet" class="setting-link">管理舰船连接</router-link></div>
          </section>
        </n-tab-pane>
        <n-tab-pane name="registries" tab="补给港"><section class="setting-section registry-setting"><RegistryManager /></section></n-tab-pane>
        <n-tab-pane name="appearance" tab="界面">
          <section class="setting-section">
            <div class="section-heading"><div><h2>舰桥外观</h2><p>主题只按你的选择切换，不跟随系统设置。</p></div></div>
            <div class="theme-options">
              <button :class="{ active: appearance.theme === 'deep' }" @click="appearance.setTheme('deep')"><span class="theme-preview deep"><i /><b /><em /></span><span><n-icon :component="Moon" />深海模式<small>深色舰桥，适合长时间专注操作</small></span><i v-if="appearance.theme === 'deep'" class="selected-dot" /></button>
              <button :class="{ active: appearance.theme === 'shallow' }" @click="appearance.setTheme('shallow')"><span class="theme-preview shallow"><i /><b /><em /></span><span><n-icon :component="Sun" />浅海模式<small>明亮工作区，适合光线充足的环境</small></span><i v-if="appearance.theme === 'shallow'" class="selected-dot" /></button>
            </div>
            <div class="setting-row"><div><strong>收起左侧导航</strong><p>导航宽度在 200px 与 56px 之间切换，并记住你的选择。</p></div><n-switch v-model:value="appearance.collapsed" /></div>
          </section>
        </n-tab-pane>
        <n-tab-pane name="logs" tab="日志巡检">
          <section class="setting-section">
            <div class="section-heading"><div><h2>航行日志</h2><p>这些选项直接应用于集装箱详情中的日志面板。</p></div></div>
            <div class="setting-row"><div><strong>默认读取行数</strong><p>从日志尾部读取 50–2000 行，避免弱网下一次加载过多内容。</p></div><n-input-number v-model:value="appearance.logTail" :min="50" :max="2000" :step="50" /></div>
            <div class="setting-row"><div><strong>默认持续追踪</strong><p>开启后每 3 秒获取最新日志，关闭面板时自动停止。</p></div><n-switch v-model:value="appearance.logFollow" /></div>
            <div class="privacy-note"><Info :size="16" /><p>连接密码、私钥和口令不会写入应用配置或展示在日志面板中。</p></div>
          </section>
        </n-tab-pane>
        <n-tab-pane name="about" tab="关于">
          <section class="setting-section about-section">
            <div class="about-brand"><Anchor :size="32" /><div><h2>ShipWiz</h2><span>舰桥控制台</span></div></div>
            <p>为船长提供清晰、集中的舰船与集装箱管理工作台。通过本机、TLS 或 SSH 安全通道操控动力引擎。</p>
            <dl><div><dt>应用版本</dt><dd>{{ version }}</dd></div><div><dt>当前版本阶段</dt><dd>桌面稳定版</dd></div><div><dt>配置保存位置</dt><dd>本机用户目录</dd></div><div><dt>许可</dt><dd>项目内部使用</dd></div></dl>
          </section>
        </n-tab-pane>
      </n-tabs>
    </div>
  </div>
</template>

<style scoped>
.settings-layout { display: grid; grid-template-columns: 180px minmax(0, 1fr); min-height: 560px; overflow: hidden; border: 1px solid var(--sw-border); border-radius: 8px; background: var(--sw-surface); }
.settings-index { display: flex; flex-direction: column; gap: 5px; padding: 20px 12px; border-right: 1px solid var(--sw-border); background: var(--sw-surface-soft); }.settings-index > span { margin: 0 10px 10px; color: var(--sw-muted); font-size: 10px; letter-spacing: 1px; }.settings-index button { display: flex; align-items: center; gap: 10px; height: 38px; padding: 0 11px; color: var(--sw-text-secondary); border: 1px solid transparent; border-radius: 7px; background: transparent; font-size: 12px; text-align: left; cursor: pointer; }.settings-index button:hover { color: var(--sw-text); background: var(--sw-surface); }.settings-index button.active { color: var(--sw-primary); border-color: var(--sw-border); background: var(--sw-surface); }
.settings-tabs { min-width: 0; padding: 0 24px 24px; }.settings-tabs :deep(.n-tabs-tab) { display: none; }.settings-tabs :deep(.n-tabs-nav) { display: none; }.settings-tabs :deep(.n-tab-pane) { padding-top: 24px; }
.setting-section { max-width: 900px; }.section-heading { display: flex; align-items: flex-start; justify-content: space-between; gap: 16px; margin-bottom: 20px; }.section-heading h2 { margin: 0; color: var(--sw-text); font-size: 17px; }.section-heading p, .setting-row p { margin: 6px 0 0; color: var(--sw-muted); font-size: 11px; line-height: 1.6; }
.setting-row { display: flex; align-items: center; justify-content: space-between; gap: 24px; min-height: 76px; padding: 15px 0; border-top: 1px solid var(--sw-border); }.setting-row strong { color: var(--sw-text); font-size: 12px; font-weight: 600; }.engine-select { width: 240px; }.engine-state { display: flex; align-items: center; gap: 8px; color: var(--sw-text-secondary); font-size: 12px; }.section-actions { display: flex; align-items: center; gap: 18px; margin-top: 22px; }.setting-link { color: var(--sw-primary); font-size: 11px; text-decoration: none; }.engine-message { padding: 10px 12px; color: var(--sw-danger); border-radius: 6px; background: var(--sw-danger-soft); font-size: 11px; }.engine-message.success { color: var(--sw-success); background: var(--sw-success-soft); }
.registry-setting { max-width: none; }.registry-setting :deep(.registry-toolbar) { padding: 0 0 18px; border: 0; }.registry-setting :deep(.registry-grid) { padding: 0; }
.theme-options { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 14px; margin-bottom: 18px; }.theme-options > button { position: relative; padding: 12px; color: var(--sw-text); border: 1px solid var(--sw-border); border-radius: 8px; background: var(--sw-surface-soft); text-align: left; cursor: pointer; }.theme-options > button.active { border-color: var(--sw-primary); box-shadow: inset 0 0 0 1px var(--sw-primary); }.theme-options > button > span:nth-child(2) { display: grid; grid-template-columns: 20px 1fr; align-items: center; gap: 3px; margin-top: 12px; font-size: 12px; }.theme-options small { grid-column: 2; margin-top: 3px; color: var(--sw-muted); font-size: 9px; }.theme-preview { display: grid; grid-template: 20px 1fr / 28px 1fr; height: 94px; overflow: hidden; border: 1px solid var(--sw-border); border-radius: 6px; }.theme-preview i { grid-column: 1 / -1; }.theme-preview b { grid-row: 2; }.theme-preview em { grid-row: 2; margin: 9px; border-radius: 3px; }.theme-preview.deep { background: #0f172a; }.theme-preview.deep i { background: #1e293b; }.theme-preview.deep b { background: #1e293b; }.theme-preview.deep em { background: #243248; }.theme-preview.shallow { background: #f7fafc; }.theme-preview.shallow i { background: #fff; }.theme-preview.shallow b { background: #edf2f7; }.theme-preview.shallow em { background: #fff; }.selected-dot { position: absolute; right: 10px; bottom: 15px; width: 7px; height: 7px; border-radius: 50%; background: var(--sw-primary); }
.privacy-note { display: flex; gap: 9px; margin-top: 20px; padding: 12px; color: var(--sw-primary); border-radius: 8px; background: var(--sw-primary-soft); }.privacy-note p { margin: 0; color: var(--sw-text-secondary); font-size: 11px; }
.about-brand { display: flex; align-items: center; gap: 14px; color: var(--sw-primary); }.about-brand h2 { margin: 0; color: var(--sw-text); font-family: "Bahnschrift", sans-serif; font-size: 26px; }.about-brand span { color: var(--sw-muted); font-size: 11px; }.about-section > p { max-width: 600px; margin: 24px 0; color: var(--sw-text-secondary); font-size: 12px; line-height: 1.8; }.about-section dl { max-width: 560px; margin: 0; }.about-section dl > div { display: flex; justify-content: space-between; padding: 13px 0; border-top: 1px solid var(--sw-border); font-size: 11px; }.about-section dt { color: var(--sw-muted); }.about-section dd { margin: 0; color: var(--sw-text); }
@media (max-width: 900px) { .settings-layout { grid-template-columns: 140px minmax(0, 1fr); }.theme-options { grid-template-columns: 1fr; }.engine-select { width: 190px; } }
</style>
