<script setup lang="ts">
import { NIcon, NTooltip } from "naive-ui";
import { CircleHelp } from "lucide-vue-next";

defineProps<{ term: string; explanation: string }>();
</script>

<template>
  <n-tooltip placement="top" :style="{ maxWidth: '280px' }">
    <template #trigger>
      <span class="term-hint" tabindex="0"
        >{{ term }}<n-icon :component="CircleHelp" :size="13"
      /></span>
    </template>
    <strong>{{ term }}</strong><br />{{ explanation }}
  </n-tooltip>
</template>

<style scoped>
.term-hint {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  border-bottom: 1px dashed var(--sw-border-strong);
  cursor: help;
}
.term-hint .n-icon {
  color: var(--sw-muted);
}
</style>
