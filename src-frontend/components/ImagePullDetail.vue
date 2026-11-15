<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { NButton, NModal, NSelect, NTag } from "naive-ui";
import { PackageOpen, Star } from "lucide-vue-next";
import { dict } from "../dict";
import { useImageStore } from "../store/image";
import type { ImageSearchResult } from "../types";

const props = defineProps<{
  show: boolean;
  nodeId: string;
  target: ImageSearchResult | null;
}>();
const emit = defineEmits<{
  "update:show": [value: boolean];
  pulled: [];
}>();

const images = useImageStore();
const tag = ref("latest");
const registryId = ref<string | null>(null);
const pulling = ref(false);

const TAG_PRESETS: Record<string, string[]> = {
  nginx: ["latest", "1.27-alpine", "1.27", "stable", "alpine"],
  mysql: ["latest", "8.4", "8.0", "5.7"],
  mariadb: ["latest", "11", "10.11"],
  redis: ["latest", "7.4-alpine", "7.2", "alpine"],
  postgres: ["latest", "17", "16", "15-alpine"],
  mongo: ["latest", "7", "6"],
  node: ["latest", "22-alpine", "20-alpine", "20"],
  python: ["latest", "3.12-slim", "3.11-slim"],
  golang: ["latest", "1.23-alpine"],
  openjdk: ["latest", "21-slim", "17-slim"],
  alpine: ["latest", "3.20", "3.19"],
  ubuntu: ["latest", "24.04", "22.04"],
  rabbitmq: ["latest", "4-management", "3-management"],
  elasticsearch: ["latest", "8.15", "7.17"],
  grafana: ["latest", "11", "10"],
  prometheus: ["latest", "v3", "v2"],
  jenkins: ["latest", "lts-jdk17"],
  minio: ["latest", "RELEASE.2024"],
};

const repository = computed(() => (props.target?.name ?? "").split(":")[0]);
const repositoryKey = computed(() => repository.value.split("/").pop() ?? repository.value);
const tagOptions = computed(() => {
  const presets = TAG_PRESETS[repositoryKey.value] ?? ["latest"];
  return presets.map((value) => ({ label: value, value }));
});
const reference = computed(() => `${repository.value}:${tag.value || "latest"}`);
const registryOptions = computed(() =>
  images.registries.map((registry) => ({
    label: registry.isDefault ? `${registry.name} · ${dict.image.registry.default}` : registry.name,
    value: registry.id,
  })),
);
const registryLabel = computed(
  () => registryOptions.value.find((option) => option.value === registryId.value)?.label ?? dict.image.registry.default,
);
const starLabel = computed(() => {
  const count = props.target?.starCount ?? 0;
  if (count >= 10000) return `${(count / 10000).toFixed(1)}w`;
  if (count >= 1000) return `${(count / 1000).toFixed(1)}k`;
  return String(count);
});

watch(
  () => [props.show, props.target?.name] as const,
  ([show]) => {
    if (!show) return;
    tag.value = (props.target?.name ?? "").includes(":") ? props.target!.name.split(":")[1] : "latest";
    registryId.value = images.registries.find((registry) => registry.isDefault)?.id ?? null;
    pulling.value = false;
  },
  { immediate: true },
);

async function submit() {
  if (pulling.value || !props.nodeId) return;
  pulling.value = true;
  const success = await images.pull(props.nodeId, reference.value, registryId.value);
  pulling.value = false;
  if (success) {
    emit("update:show", false);
    emit("pulled");
  }
}
</script>

<template>
  <n-modal
    :show="show"
    preset="card"
    :title="dict.image.market.detailTitle"
    class="form-modal market-detail-modal"
    :mask-closable="!pulling"
    :closable="!pulling"
    @update:show="emit('update:show', $event)"
  >
    <div v-if="target" class="detail-head">
      <span class="detail-icon"><PackageOpen :size="26" /></span>
      <div class="detail-copy">
        <h3>{{ repository }}</h3>
        <div class="detail-meta">
          <n-tag size="small" :bordered="false" :type="target.isOfficial ? 'success' : 'default'">
            {{ target.isOfficial ? dict.image.official : dict.image.community }}
          </n-tag>
          <span class="detail-stars"><Star :size="12" />{{ starLabel }} {{ dict.image.stars }}</span>
        </div>
      </div>
    </div>

    <p class="detail-desc">{{ target?.description || dict.image.noDescription }}</p>

    <div class="detail-fields">
      <div class="detail-field">
        <label>{{ dict.image.market.tagLabel }}</label>
        <n-select
          v-model:value="tag"
          :options="tagOptions"
          :disabled="pulling"
          filterable
          tag
          :placeholder="dict.image.market.tagPlaceholder"
        />
        <span class="detail-hint">{{ dict.image.market.customTag }}</span>
      </div>
      <div class="detail-field">
        <label>{{ dict.image.pullModal.registry }}</label>
        <n-select v-model:value="registryId" :options="registryOptions" :disabled="pulling" />
      </div>
    </div>

    <div class="detail-reference">
      <span>{{ dict.image.market.willPull }}</span>
      <code>{{ reference }}</code>
      <em>{{ registryLabel }}</em>
    </div>

    <div class="modal-actions">
      <n-button :disabled="pulling" @click="emit('update:show', false)">
        {{ dict.common.cancel }}
      </n-button>
      <n-button type="primary" :loading="pulling" @click="submit">
        {{ pulling ? dict.image.pullModal.pulling : dict.image.market.fetch }}
      </n-button>
    </div>
  </n-modal>
</template>

<style scoped>
.market-detail-modal { width: 520px; }
.detail-head { display: flex; align-items: center; gap: 14px; }
.detail-icon { display: grid; place-items: center; flex: 0 0 auto; width: 48px; height: 48px; color: var(--sw-primary); border: 1px solid var(--sw-border); border-radius: 12px; background: var(--sw-primary-soft); }
.detail-copy { min-width: 0; }
.detail-copy h3 { overflow: hidden; margin: 0; color: var(--sw-text); font-family: Consolas, monospace; font-size: 14px; font-weight: 600; text-overflow: ellipsis; white-space: nowrap; }
.detail-meta { display: flex; align-items: center; gap: 8px; margin-top: 7px; }
.detail-stars { display: inline-flex; align-items: center; gap: 4px; color: var(--sw-warning); font-size: 10px; }
.detail-desc { margin: 16px 0 18px; color: var(--sw-text-secondary); font-size: 12px; line-height: 1.75; }
.detail-field { margin-bottom: 14px; }
.detail-field label { display: block; margin-bottom: 7px; color: var(--sw-text-secondary); font-size: 11px; }
.detail-hint { display: block; margin-top: 6px; color: var(--sw-muted); font-size: 10px; }
.detail-reference { display: flex; align-items: center; flex-wrap: wrap; gap: 8px; margin: 4px 0 20px; padding: 10px 12px; border: 1px solid var(--sw-border); border-radius: 8px; background: var(--sw-surface-soft); font-size: 11px; }
.detail-reference span { color: var(--sw-muted); }
.detail-reference code { color: var(--sw-text); font-family: Consolas, monospace; font-size: 12px; overflow-wrap: anywhere; }
.detail-reference em { margin-left: auto; color: var(--sw-muted); font-style: normal; font-size: 10px; }
.modal-actions { display: flex; justify-content: flex-end; gap: 10px; }
</style>
