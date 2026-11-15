<script setup lang="ts">
import { computed, ref } from "vue";
import { NButton, NIcon } from "naive-ui";
import {
  Anchor,
  ArrowRight,
  Box,
  Check,
  FileStack,
  Ship,
  X,
} from "lucide-vue-next";
import { useFleetStore } from "../store/fleet";

const emit = defineEmits<{ finished: [] }>();
const fleet = useFleetStore();
const step = ref(0);
const checking = ref(false);
const error = ref("");
const steps = [
  {
    icon: Ship,
    title: "先选一艘舰船",
    copy: "舰队总览展示每艘舰船的连接状态。当前舰船决定后续操作的目的地。",
  },
  {
    icon: Box,
    title: "让服务平稳航行",
    copy: "在集装箱管理中起锚、抛锚、查看日志或登舱巡检，无需输入命令。",
  },
  {
    icon: FileStack,
    title: "用编队协同起航",
    copy: "图纸舱准备物料，编队指令负责组织多个服务，动力监控台持续观察状态。",
  },
];
const currentStep = computed(() => steps[Math.max(0, step.value - 1)]);

function finish() {
  localStorage.setItem("shipwiz.onboarded", "true");
  emit("finished");
}
async function launch() {
  const node =
    fleet.currentNode ||
    fleet.state.nodes.find((item) => item.type === "local");
  if (!node) {
    step.value = 1;
    return;
  }
  checking.value = true;
  error.value = "";
  const ok = await fleet.test(node.id);
  checking.value = false;
  if (ok) {
    step.value = 1;
  } else {
    error.value =
      "暂未探测到本机动力引擎。请确认引擎已启动并允许当前用户访问，再继续；也可以先添加远程舰船。";
  }
}
</script>

<template>
  <div
    class="onboarding-backdrop"
    role="dialog"
    aria-modal="true"
    aria-label="ShipWiz 新手引导"
  >
    <section class="onboarding-card">
      <button class="skip-icon" aria-label="跳过引导" @click="finish">
        <n-icon :component="X" :size="18" />
      </button>
      <template v-if="step === 0">
        <div class="welcome-emblem">
          <Anchor :size="34" :stroke-width="1.5" />
        </div>
        <span class="welcome-kicker">WELCOME ABOARD</span>
        <h1>欢迎登舰，船长</h1>
        <p class="welcome-copy">
          ShipWiz
          把引擎、集装箱与编队收进一座清晰的中文舰桥。先探测本机动力，即可开始第一段航程。
        </p>
        <div class="launch-status">
          <span
            class="status-light"
            :class="fleet.currentNode?.status || 'offline'"
          />{{ fleet.currentNode?.name || "本机舰船"
          }}<small>自动探测系统连接通道</small>
        </div>
        <p v-if="error" class="launch-error" role="alert">{{ error }}</p>
        <n-button
          type="primary"
          size="large"
          block
          :loading="checking"
          @click="launch"
        >
          一键启航<template #icon><n-icon :component="ArrowRight" /></template>
        </n-button>
        <div class="welcome-secondary">
          <button @click="step = 1">先看看舰桥</button>
          <button @click="finish">跳过引导</button>
        </div>
      </template>
      <template v-else>
        <span class="tour-count"
          >舰桥导览 · {{ step }} / {{ steps.length }}</span
        >
        <div class="tour-icon">
          <n-icon :component="currentStep.icon" :size="28" />
        </div>
        <h2>{{ currentStep.title }}</h2>
        <p>{{ currentStep.copy }}</p>
        <div class="tour-progress">
          <span
            v-for="index in steps.length"
            :key="index"
            :class="{ active: index <= step }"
          />
        </div>
        <div class="tour-actions">
          <n-button quaternary @click="finish">跳过</n-button>
          <n-button v-if="step < steps.length" type="primary" @click="step += 1"
            >下一站<n-icon :component="ArrowRight"
          /></n-button>
          <n-button v-else type="primary" @click="finish"
            ><n-icon :component="Check" />开始航行</n-button
          >
        </div>
      </template>
    </section>
  </div>
</template>

<style scoped>
.onboarding-backdrop {
  position: fixed;
  z-index: 1000;
  inset: 0;
  display: grid;
  place-items: center;
  padding: 24px;
  background: rgba(3, 8, 18, 0.76);
  backdrop-filter: blur(8px);
}
.onboarding-card {
  position: relative;
  width: min(480px, 100%);
  padding: 36px;
  color: var(--sw-text);
  border: 1px solid var(--sw-border-strong);
  border-radius: 12px;
  background: var(--sw-surface);
  box-shadow: 0 30px 90px rgba(0, 0, 0, 0.35);
}
.skip-icon {
  position: absolute;
  top: 14px;
  right: 14px;
  display: grid;
  place-items: center;
  width: 30px;
  height: 30px;
  color: var(--sw-muted);
  border: 0;
  border-radius: 6px;
  background: transparent;
  cursor: pointer;
}
.skip-icon:hover {
  color: var(--sw-text);
  background: var(--sw-surface-soft);
}
.welcome-emblem {
  display: grid;
  place-items: center;
  width: 64px;
  height: 64px;
  margin-bottom: 25px;
  color: #fff;
  border-radius: 14px;
  background: var(--sw-primary);
}
.welcome-kicker,
.tour-count {
  color: var(--sw-primary);
  font-family: "Bahnschrift", sans-serif;
  font-size: 10px;
  font-weight: 600;
  letter-spacing: 1.8px;
}
h1 {
  margin: 9px 0 12px;
  font-family: "Bahnschrift", "Microsoft YaHei UI", sans-serif;
  font-size: 30px;
}
.welcome-copy,
.onboarding-card > p {
  color: var(--sw-text-secondary);
  font-size: 13px;
  line-height: 1.8;
}
.launch-status {
  display: flex;
  align-items: center;
  gap: 9px;
  margin: 24px 0 16px;
  padding: 13px;
  border: 1px solid var(--sw-border);
  border-radius: 8px;
  background: var(--sw-surface-soft);
  font-size: 12px;
}
.launch-status small {
  margin-left: auto;
  color: var(--sw-muted);
  font-size: 10px;
}
.launch-error {
  padding: 10px 12px;
  color: var(--sw-danger) !important;
  border-radius: 6px;
  background: var(--sw-danger-soft);
  font-size: 11px !important;
}
.welcome-secondary {
  display: flex;
  justify-content: space-between;
  margin-top: 16px;
}
.welcome-secondary button {
  color: var(--sw-muted);
  border: 0;
  background: transparent;
  font-size: 11px;
  cursor: pointer;
}
.welcome-secondary button:hover {
  color: var(--sw-text);
}
.tour-icon {
  display: grid;
  place-items: center;
  width: 56px;
  height: 56px;
  margin: 26px 0 20px;
  color: var(--sw-primary);
  border: 1px solid var(--sw-border);
  border-radius: 12px;
  background: var(--sw-primary-soft);
}
h2 {
  margin: 0 0 10px;
  font-size: 22px;
}
.tour-progress {
  display: flex;
  gap: 6px;
  margin: 28px 0 20px;
}
.tour-progress span {
  width: 24px;
  height: 3px;
  border-radius: 2px;
  background: var(--sw-border);
}
.tour-progress span.active {
  background: var(--sw-primary);
}
.tour-actions {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
</style>
