<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref, watch } from "vue";
import { NButton, NIcon } from "naive-ui";
import { Eraser, RefreshCw, TerminalSquare } from "lucide-vue-next";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";
import { decodeTerminalOutput, terminalApi } from "../api/terminal";

const props = defineProps<{
  nodeId: string;
  containerId: string;
  active: boolean;
}>();
const host = ref<HTMLDivElement | null>(null);
const sessionId = ref("");
const connecting = ref(false);
const disconnected = ref(true);
const statusMessage = ref("正在建立安全巡检通道…");
let terminal: Terminal | undefined;
let fitAddon: FitAddon | undefined;
let observer: ResizeObserver | undefined;
let listenersReady: Promise<void> | undefined;
let unlistenOutput: (() => void) | undefined;
let unlistenStatus: (() => void) | undefined;
let lastSequence = -1;
let opening = 0;
let disposed = false;

function initialize() {
  if (!host.value || terminal) return;
  terminal = new Terminal({
    fontFamily: '"Cascadia Code", "Microsoft YaHei UI", monospace',
    fontSize: 13,
    lineHeight: 1.5,
    cursorBlink: true,
    scrollback: 5000,
    theme: {
      background: "#08111F",
      foreground: "#D8E5F2",
      cursor: "#72A5FF",
      selectionBackground: "#26456F",
    },
  });
  fitAddon = new FitAddon();
  terminal.loadAddon(fitAddon);
  terminal.open(host.value);
  terminal.onData((data) => {
    if (!sessionId.value || disconnected.value) return;
    const id = sessionId.value;
    void terminalApi.input(id, data).catch((error: unknown) => {
      if (sessionId.value !== id) return;
      disconnected.value = true;
      connecting.value = false;
      statusMessage.value =
        error instanceof Error
          ? error.message
          : "发送终端输入失败，请重新连接。";
    });
  });
  terminal.onResize(({ cols, rows }) => {
    if (sessionId.value && cols > 0 && rows > 0) {
      void terminalApi
        .resize(sessionId.value, cols, rows)
        .catch(() => undefined);
    }
  });
  observer = new ResizeObserver(() => fitAddon?.fit());
  observer.observe(host.value);
  listenersReady = Promise.all([
    terminalApi.onOutput((event) => {
      if (event.sessionId !== sessionId.value || event.seq <= lastSequence)
        return;
      lastSequence = event.seq;
      terminal?.write(decodeTerminalOutput(event.data));
    }),
    terminalApi.onStatus((event) => {
      if (event.sessionId !== sessionId.value) return;
      if (event.status === "open") {
        disconnected.value = false;
        statusMessage.value = "";
        return;
      }
      disconnected.value = true;
      connecting.value = false;
      statusMessage.value =
        event.message ||
        (event.status === "closed"
          ? "巡检会话已结束。"
          : "连接已中断，请重新建立巡检会话。");
    }),
  ]).then(([output, status]) => {
    unlistenOutput = output;
    unlistenStatus = status;
    if (disposed) {
      output();
      status();
    }
  });
}

async function open() {
  if (
    disposed ||
    !props.active ||
    !props.nodeId ||
    !props.containerId ||
    !host.value
  )
    return;
  await close();
  if (
    disposed ||
    !props.active ||
    !props.nodeId ||
    !props.containerId ||
    !host.value
  )
    return;
  const ticket = ++opening;
  initialize();
  await listenersReady;
  if (ticket !== opening || disposed || !props.active) return;
  const id = crypto.randomUUID();
  sessionId.value = id;
  lastSequence = -1;
  connecting.value = true;
  disconnected.value = false;
  statusMessage.value = "正在建立安全巡检通道…";
  terminal?.reset();
  await nextTick();
  fitAddon?.fit();
  try {
    const openedId = await terminalApi.open(
      props.nodeId,
      props.containerId,
      terminal?.cols || 80,
      terminal?.rows || 24,
      id,
    );
    if (ticket !== opening || disposed || sessionId.value !== id) {
      void terminalApi.close(openedId).catch(() => undefined);
      return;
    }
    sessionId.value = openedId;
    disconnected.value = false;
    statusMessage.value = "";
    terminal?.focus();
  } catch (error) {
    if (ticket === opening) {
      sessionId.value = "";
      disconnected.value = true;
      statusMessage.value =
        error instanceof Error ? error.message : "巡检通道连接失败，请重试。";
    }
  } finally {
    if (ticket === opening) connecting.value = false;
  }
}

async function close() {
  opening += 1;
  const id = sessionId.value;
  sessionId.value = "";
  connecting.value = false;
  disconnected.value = true;
  lastSequence = -1;
  if (id) await terminalApi.close(id).catch(() => undefined);
}

function reconnect() {
  void open();
}

watch(
  () => [props.active, props.nodeId, props.containerId] as const,
  ([active]) => {
    if (active) void nextTick(open);
    else void close();
  },
  { immediate: true, flush: "post" },
);

onBeforeUnmount(() => {
  disposed = true;
  void close();
  observer?.disconnect();
  unlistenOutput?.();
  unlistenStatus?.();
  terminal?.dispose();
});
</script>

<template>
  <section class="interactive-terminal">
    <header>
      <div>
        <TerminalSquare :size="14" /><span>交互巡检</span
        ><small>会话在当前集装箱内运行</small>
      </div>
      <div>
        <n-button
          quaternary
          size="small"
          :disabled="!sessionId"
          @click="terminal?.clear()"
        >
          <template #icon><n-icon :component="Eraser" /></template>清屏
        </n-button>
        <n-button
          quaternary
          size="small"
          :loading="connecting"
          :disabled="!active"
          @click="reconnect"
        >
          <template #icon><n-icon :component="RefreshCw" /></template>重连
        </n-button>
      </div>
    </header>
    <div class="terminal-viewport">
      <div ref="host" class="terminal-host" />
      <div v-if="disconnected" class="terminal-overlay">
        <TerminalSquare :size="28" />
        <strong>{{ connecting ? "正在连接" : "巡检会话未连接" }}</strong>
        <p>{{ statusMessage }}</p>
        <n-button
          size="small"
          type="primary"
          :loading="connecting"
          :disabled="!active"
          @click="reconnect"
          >建立连接</n-button
        >
      </div>
    </div>
    <footer>
      输入将直接发送至集装箱；重连会创建新会话，不恢复旧会话状态。
    </footer>
  </section>
</template>

<style scoped>
.interactive-terminal {
  overflow: hidden;
  border: 1px solid var(--sw-border);
  border-radius: 8px;
  background: var(--sw-terminal-bg);
}
.interactive-terminal header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  min-height: 44px;
  padding: 5px 9px 5px 13px;
  color: var(--sw-terminal-text);
  border-bottom: 1px solid #243248;
  background: #101c2e;
}
.interactive-terminal header > div {
  display: flex;
  align-items: center;
  gap: 7px;
}
.interactive-terminal header span {
  font-size: 11px;
  font-weight: 600;
}
.interactive-terminal header small {
  color: #8190a3;
  font-size: 9px;
}
.terminal-viewport {
  position: relative;
  height: min(460px, calc(100vh - 270px));
  min-height: 300px;
  padding: 10px;
  background: var(--sw-terminal-bg);
}
.terminal-host {
  width: 100%;
  height: 100%;
}
.terminal-host :deep(.xterm) {
  height: 100%;
}
.terminal-host :deep(.xterm-viewport) {
  overflow-y: auto !important;
}
.terminal-overlay {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-direction: column;
  padding: 24px;
  color: #a8b6c9;
  background: rgba(8, 17, 31, 0.9);
  text-align: center;
}
.terminal-overlay strong {
  margin-top: 12px;
  color: #e5edf8;
  font-size: 13px;
}
.terminal-overlay p {
  max-width: 360px;
  margin: 8px 0 16px;
  color: #a8b6c9;
  font-size: 11px;
  line-height: 1.6;
}
.interactive-terminal footer {
  padding: 8px 13px;
  color: #8190a3;
  border-top: 1px solid #243248;
  background: #101c2e;
  font-size: 9px;
}
@media (max-width: 600px) {
  .interactive-terminal header small {
    display: none;
  }
}
</style>
