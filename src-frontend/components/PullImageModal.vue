<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { NButton, NForm, NFormItem, NInput, NModal, NSelect } from "naive-ui";
import { message } from "../utils/message";
import { dict } from "../dict";
import { useImageStore } from "../store/image";

const props = defineProps<{
  show: boolean;
  nodeId: string;
  reference?: string;
}>();
const emit = defineEmits<{
  "update:show": [value: boolean];
  pulled: [];
}>();
const images = useImageStore();
const reference = ref("");
const registryId = ref<string | null>(null);
const submitting = ref(false);
const registryOptions = computed(() =>
  images.registries.map((registry) => ({
    label: registry.isDefault ? `${registry.name} · ${dict.image.registry.default}` : registry.name,
    value: registry.id,
  })),
);

watch(
  () => props.show,
  (show) => {
    if (!show) return;
    reference.value = props.reference ?? "";
    registryId.value = images.registries.find((registry) => registry.isDefault)?.id ?? null;
  },
);

async function submit() {
  if (!reference.value.trim()) {
    message.warning(dict.image.pullModal.required);
    return;
  }
  submitting.value = true;
  const success = await images.pull(props.nodeId, reference.value.trim(), registryId.value);
  submitting.value = false;
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
    :title="dict.image.pullModal.title"
    class="form-modal pull-image-modal"
    :mask-closable="!submitting"
    :closable="!submitting"
    @update:show="emit('update:show', $event)"
  >
    <n-form label-placement="top" @submit.prevent="submit">
      <n-form-item :label="dict.image.pullModal.reference" required>
        <n-input
          v-model:value="reference"
          :placeholder="dict.image.pullModal.referencePlaceholder"
          :disabled="submitting"
          autofocus
          @keyup.enter="submit"
        />
      </n-form-item>
      <n-form-item :label="dict.image.pullModal.registry">
        <n-select
          v-model:value="registryId"
          :options="registryOptions"
          :disabled="submitting"
        />
      </n-form-item>
      <p class="pull-hint">{{ dict.image.pullModal.hint }}</p>
      <div class="modal-actions">
        <n-button :disabled="submitting" @click="emit('update:show', false)">
          {{ dict.common.cancel }}
        </n-button>
        <n-button type="primary" :loading="submitting" @click="submit">
          {{ submitting ? dict.image.pullModal.pulling : dict.image.pullModal.submit }}
        </n-button>
      </div>
    </n-form>
  </n-modal>
</template>

<style scoped>
.pull-image-modal { width: 500px; max-width: calc(100vw - 40px); }
.pull-hint { margin: -4px 0 20px; color: #879a9b; font-size: 11px; line-height: 1.7; }
.modal-actions { display: flex; justify-content: flex-end; gap: 10px; }
</style>
