<script setup lang="ts">
import { NProgress } from "naive-ui";
import { dict } from "../dict";
import { useImageStore } from "../store/image";

const images = useImageStore();

function formatBytes(bytes: number) {
  if (bytes < 1024 * 1024) return `${Math.max(0, Math.round(bytes / 1024))} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}
</script>

<template>
  <section class="image-pull-progress" aria-live="polite">
    <div class="pull-summary">
      <span>{{ images.pullStatus }}</span>
      <strong v-if="images.pullPercent !== null"
        >{{ images.pullPercent }}%</strong
      >
    </div>
    <n-progress
      v-if="images.pullPercent !== null"
      type="line"
      :percentage="images.pullPercent"
      :height="8"
    />
    <p v-else class="progress-note">
      {{ dict.image.pullModal.progressUnknown }}
    </p>
    <p v-if="images.pullTotal > 0" class="progress-bytes">
      {{ formatBytes(images.pullCurrent) }} /
      {{ formatBytes(images.pullTotal) }}
    </p>
    <div v-if="images.pullLayers.length" class="layer-list">
      <div
        v-for="layer in images.pullLayers"
        :key="layer.layerId"
        class="layer-row"
      >
        <code>{{ layer.layerId }}</code>
        <span>{{ layer.status }}</span>
        <small v-if="layer.total && layer.total > 0">
          {{ formatBytes(layer.current ?? 0) }} / {{ formatBytes(layer.total) }}
        </small>
        <small v-else>{{ dict.image.pullModal.unknownSize }}</small>
      </div>
    </div>
  </section>
</template>

<style scoped>
.image-pull-progress {
  margin: 4px 0 18px;
  padding: 12px;
  border: 1px solid var(--sw-border);
  border-radius: 8px;
  background: var(--sw-surface-soft);
}
.pull-summary {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 9px;
  color: var(--sw-text);
  font-size: 11px;
}
.pull-summary span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.pull-summary strong {
  color: var(--sw-primary);
  font-family: Consolas, monospace;
}
.progress-note,
.progress-bytes {
  margin: 8px 0 0;
  color: var(--sw-muted);
  font-size: 10px;
  line-height: 1.6;
}
.progress-bytes {
  font-family: Consolas, monospace;
  text-align: right;
}
.layer-list {
  max-height: 180px;
  margin-top: 10px;
  overflow-y: auto;
  border-top: 1px solid var(--sw-border);
}
.layer-row {
  display: grid;
  grid-template-columns: minmax(80px, 0.8fr) minmax(100px, 1fr) auto;
  gap: 10px;
  align-items: center;
  padding: 7px 0;
  border-bottom: 1px solid var(--sw-border);
  font-size: 10px;
}
.layer-row:last-child {
  border-bottom: 0;
}
.layer-row code {
  overflow: hidden;
  color: var(--sw-text-secondary);
  text-overflow: ellipsis;
  white-space: nowrap;
}
.layer-row span {
  color: var(--sw-text);
}
.layer-row small {
  color: var(--sw-muted);
  font-family: Consolas, monospace;
  white-space: nowrap;
}
</style>
