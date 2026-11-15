<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { RouterLink, RouterView, useRoute } from "vue-router";
import {
  NConfigProvider,
  NGlobalStyle,
  NIcon,
  NTooltip,
  darkTheme,
  type GlobalThemeOverrides,
} from "naive-ui";
import {
  Activity,
  Anchor,
  Box,
  ChevronLeft,
  ChevronRight,
  FileStack,
  Images,
  Moon,
  Settings,
  Ship,
  Sun,
} from "lucide-vue-next";
import { dict } from "./dict";
import { useAppearanceStore } from "./store/appearance";
import { useFleetStore } from "./store/fleet";
import { syncMessageTheme, alertWarning } from "./utils/message";
import OnboardingTour from "./components/OnboardingTour.vue";
import { checkUpdate } from "./api/update";

const route = useRoute();
const fleet = useFleetStore();
const appearance = useAppearanceStore();
const showOnboarding = ref(false);
const navigation = computed(() => [
  {
    name: "fleet",
    path: "/fleet",
    label: dict.nav.fleet,
    icon: Ship,
    hint: "管理与切换舰船",
  },
  {
    name: "container",
    path: "/container",
    label: dict.nav.container,
    icon: Box,
    hint: "查看集装箱与服务状态",
  },
  {
    name: "image",
    path: "/image",
    label: dict.nav.image,
    icon: Images,
    hint: "采购与维护货箱图纸",
  },
  {
    name: "compose",
    path: "/compose",
    label: dict.nav.compose,
    icon: FileStack,
    hint: "编排多服务编队",
  },
  {
    name: "monitor",
    path: "/monitor",
    label: dict.nav.monitor,
    icon: Activity,
    hint: "查看实时资源与异常",
  },
  {
    name: "setting",
    path: "/setting",
    label: dict.nav.setting,
    icon: Settings,
    hint: "调整引擎、补给港与界面",
  },
]);
const currentNavigation = computed(() =>
  navigation.value.find((item) => item.name === route.name),
);
const themeOverrides = computed<GlobalThemeOverrides>(() => ({
  common: {
    primaryColor: "#2F7DFF",
    primaryColorHover: "#5294FF",
    primaryColorPressed: "#165DFF",
    primaryColorSuppl: "#2F7DFF",
    successColor: "#22C55E",
    warningColor: "#F59E0B",
    errorColor: "#EF4444",
    borderRadius: "8px",
    fontFamily: '"Aptos", "Microsoft YaHei UI", "Microsoft YaHei", sans-serif',
    fontSize: "13px",
    heightMedium: "32px",
  },
  Card: { borderRadius: "8px" },
  Button: { heightMedium: "32px", heightLarge: "36px" },
  Input: { heightMedium: "32px" },
  DataTable: { thHeightMedium: "40px", tdHeightMedium: "40px" },
}));

watch(() => appearance.theme, syncMessageTheme, { immediate: true });

onMounted(async () => {
  await fleet.load();
  showOnboarding.value = localStorage.getItem("shipwiz.onboarded") !== "true";
  // 启动后静默检查更新：网络异常不打扰用户，发现新版本再提示
  checkUpdate()
    .then((info) => {
      if (info)
        alertWarning(`发现新版本 v${info.latest}，可在「设置 · 关于」查看`, {
          closable: true,
          duration: 7000,
        });
    })
    .catch(() => {});
});
</script>

<template>
  <n-config-provider
    :theme="appearance.theme === 'deep' ? darkTheme : null"
    :theme-overrides="themeOverrides"
    style="height: 100%"
  >
    <n-global-style />
    <div
      class="app-shell"
      :class="{ 'sidebar-collapsed': appearance.collapsed }"
    >
      <header class="bridge-bar">
        <RouterLink to="/fleet" class="brand" aria-label="ShipWiz 舰队总览">
          <span class="brand-mark"
            ><Anchor :size="20" :stroke-width="1.8"
          /></span>
          <span class="brand-word">ShipWiz<small>舰桥控制台</small></span>
        </RouterLink>
        <div class="bridge-context">
          <span class="context-kicker">COMMAND DECK</span>
          <strong>{{ currentNavigation?.label || dict.app.workspace }}</strong>
        </div>
        <div class="bridge-actions">
          <div
            v-if="fleet.currentNode"
            class="current-ship"
            :title="fleet.currentNode.name"
          >
            <span class="status-light" :class="fleet.currentNode.status" />
            <span>{{ fleet.currentNode.name }}</span>
            <small>{{
              fleet.currentNode.status === "online"
                ? dict.common.online
                : dict.common.offline
            }}</small>
          </div>
          <n-tooltip placement="bottom">
            <template #trigger>
              <button
                class="bridge-icon-button"
                :aria-label="
                  appearance.theme === 'deep' ? '切换浅海模式' : '切换深海模式'
                "
                @click="
                  appearance.setTheme(
                    appearance.theme === 'deep' ? 'shallow' : 'deep',
                  )
                "
              >
                <n-icon
                  :component="appearance.theme === 'deep' ? Sun : Moon"
                  :size="17"
                />
              </button>
            </template>
            {{ appearance.theme === "deep" ? "切换浅海模式" : "切换深海模式" }}
          </n-tooltip>
        </div>
      </header>

      <aside class="sidebar" :aria-label="dict.app.workspace">
        <nav class="main-nav">
          <span v-if="!appearance.collapsed" class="nav-section-label"
            >航行工作区</span
          >
          <n-tooltip
            v-for="item in navigation"
            :key="item.name"
            placement="right"
            :disabled="!appearance.collapsed"
          >
            <template #trigger>
              <RouterLink
                :to="item.path"
                class="nav-item"
                :class="{ active: route.name === item.name }"
                :aria-label="item.label"
                :title="appearance.collapsed ? item.label : undefined"
              >
                <n-icon :component="item.icon" :size="18" />
                <span class="nav-copy"
                  ><span>{{ item.label }}</span
                  ><small>{{ item.hint }}</small></span
                >
                <span
                  v-if="!appearance.collapsed && route.name === item.name"
                  class="nav-active-dot"
                />
              </RouterLink>
            </template>
            <strong>{{ item.label }}</strong><br />{{ item.hint }}
          </n-tooltip>
        </nav>
        <div class="sidebar-footer">
          <div v-if="!appearance.collapsed" class="ship-status">
            <span
              class="status-light"
              :class="fleet.currentNode?.status || 'offline'"
            />
            <div>
              <strong>{{
                fleet.currentNode?.name || dict.common.untitled
              }}</strong><small>当前操控舰船</small>
            </div>
          </div>
          <button
            class="collapse-button"
            :aria-label="appearance.collapsed ? '展开导航' : '收起导航'"
            @click="appearance.collapsed = !appearance.collapsed"
          >
            <n-icon
              :component="appearance.collapsed ? ChevronRight : ChevronLeft"
              :size="16"
            />
            <span v-if="!appearance.collapsed">收起导航</span>
          </button>
        </div>
      </aside>

      <main class="main-content">
        <RouterView />
      </main>
    </div>
    <OnboardingTour v-if="showOnboarding" @finished="showOnboarding = false" />
  </n-config-provider>
</template>
