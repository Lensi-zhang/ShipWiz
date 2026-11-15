<script setup lang="ts">
import { computed, ref, watch } from "vue";
import {
  NAutoComplete,
  NButton,
  NForm,
  NFormItem,
  NIcon,
  NInput,
  NInputNumber,
  NModal,
  NSelect,
  NSwitch,
  NTabPane,
  NTabs,
  type FormInst,
  type FormRules,
} from "naive-ui";
import { ArrowRight, Plus, Trash2 } from "lucide-vue-next";
import { dict } from "../dict";
import { useContainerStore } from "../store/container";
import { useImageStore } from "../store/image";
import type { CreateContainerInput } from "../types";

interface CreateFormModel extends Omit<CreateContainerInput, "env"> {
  envText: string;
}

const props = defineProps<{ show: boolean; nodeId: string }>();
const emit = defineEmits<{
  "update:show": [value: boolean];
  created: [];
}>();
const containers = useContainerStore();
const images = useImageStore();
const formRef = ref<FormInst | null>(null);
const creating = ref(false);
const activeTab = ref("basic");
const visible = computed({
  get: () => props.show,
  set: (value) => emit("update:show", value),
});

function emptyForm(): CreateFormModel {
  return {
    name: "",
    image: "",
    command: "",
    envText: "",
    ports: [],
    volumes: [],
    restartPolicy: "no",
    network: "bridge",
    autoRemove: false,
    startAfterCreate: true,
  };
}

const model = ref<CreateFormModel>(emptyForm());
const text = dict.container.createModal;
const rules: FormRules = {
  name: { required: true, message: text.nameRequired, trigger: ["blur", "input"] },
  image: { required: true, message: text.imageRequired, trigger: ["blur", "input"] },
};
const protocolOptions = [
  { label: "TCP", value: "tcp" },
  { label: "UDP", value: "udp" },
  { label: "SCTP", value: "sctp" },
];
const restartOptions = computed(() =>
  Object.entries(text.restartOptions).map(([value, label]) => ({ value, label })),
);
const imageOptions = computed(() => {
  const keyword = (model.value.image ?? "").trim().toLowerCase();
  const references = new Set<string>();
  for (const image of images.items) {
    for (const reference of image.repoTags) {
      if (reference === "<none>:<none>") continue;
      if (keyword && !reference.toLowerCase().includes(keyword)) continue;
      references.add(reference);
    }
  }
  return Array.from(references)
    .sort()
    .slice(0, 12)
    .map((reference) => ({ label: reference, value: reference }));
});

watch(
  () => props.show,
  (show) => {
    if (!show) return;
    model.value = emptyForm();
    activeTab.value = "basic";
    formRef.value?.restoreValidation();
    if (props.nodeId) void images.load(props.nodeId);
  },
);

function addPort() {
  model.value.ports.push({ hostPort: 8080, containerPort: 80, protocol: "tcp", hostIp: "" });
}
function addVolume() {
  model.value.volumes.push({ source: "", target: "", readOnly: false });
}
function onRestartPolicyChange(value: string) {
  model.value.restartPolicy = value;
  if (value !== "no") model.value.autoRemove = false;
}

async function submit() {
  try {
    await formRef.value?.validate();
  } catch {
    activeTab.value = "basic";
    return;
  }
  creating.value = true;
  const input: CreateContainerInput = {
    name: model.value.name.trim(),
    image: (model.value.image ?? "").trim(),
    command: model.value.command.trim(),
    env: model.value.envText
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter(Boolean),
    ports: model.value.ports,
    volumes: model.value.volumes.map((volume) => ({
      ...volume,
      source: volume.source.trim(),
      target: volume.target.trim(),
    })),
    restartPolicy: model.value.restartPolicy,
    network: model.value.network.trim(),
    autoRemove: model.value.autoRemove,
    startAfterCreate: model.value.startAfterCreate,
  };
  const id = await containers.create(props.nodeId, input);
  creating.value = false;
  if (id) {
    visible.value = false;
    emit("created");
  }
}
</script>

<template>
  <n-modal
    v-model:show="visible"
    preset="card"
    class="form-modal form-modal--wide create-container-modal"
    :title="text.title"
    :mask-closable="!creating"
  >
    <n-form
      ref="formRef"
      :model="model"
      :rules="rules"
      label-placement="top"
      require-mark-placement="right-hanging"
    >
      <n-tabs v-model:value="activeTab" type="line" animated display-directive="show">
        <n-tab-pane name="basic" :tab="text.basic">
          <div class="form-grid">
            <n-form-item :label="text.name" path="name">
              <n-input v-model:value="model.name" :placeholder="text.namePlaceholder" maxlength="63" />
            </n-form-item>
            <n-form-item :label="text.image" path="image">
              <div class="image-field">
                <n-auto-complete
                  v-model:value="model.image"
                  :options="imageOptions"
                  :placeholder="text.imagePlaceholder"
                  :loading="images.loading"
                  show-empty
                  clearable
                >
                  <template #empty>
                    <span class="auto-empty">
                      {{ images.items.length ? text.imageEmpty : text.imageEmptyLibrary }}
                    </span>
                  </template>
                </n-auto-complete>
                <span class="field-hint">{{ text.imageHint }}</span>
              </div>
            </n-form-item>
          </div>
          <n-form-item :label="text.command" path="command">
            <n-input v-model:value="model.command" :placeholder="text.commandPlaceholder" />
          </n-form-item>
          <n-form-item :label="text.env" path="envText">
            <n-input
              v-model:value="model.envText"
              type="textarea"
              :autosize="{ minRows: 5, maxRows: 9 }"
              :placeholder="text.envPlaceholder"
              class="code-input"
            />
          </n-form-item>
        </n-tab-pane>

        <n-tab-pane name="routes" :tab="text.routes">
          <section class="config-section">
            <div class="section-heading">
              <span>{{ text.ports }}</span>
              <n-button size="small" dashed @click="addPort">
                <template #icon><n-icon><Plus /></n-icon></template>
                {{ text.addPort }}
              </n-button>
            </div>
            <div v-for="(port, index) in model.ports" :key="index" class="mapping-row port-row">
              <n-input-number
                v-model:value="port.hostPort"
                :min="1"
                :max="65535"
                :show-button="false"
                :placeholder="text.hostPort"
              />
              <ArrowRight class="mapping-arrow" :size="15" />
              <n-input-number
                v-model:value="port.containerPort"
                :min="1"
                :max="65535"
                :show-button="false"
                :placeholder="text.containerPort"
              />
              <n-select v-model:value="port.protocol" :options="protocolOptions" />
              <n-button quaternary circle size="small" :aria-label="dict.common.delete" @click="model.ports.splice(index, 1)">
                <template #icon><n-icon><Trash2 /></n-icon></template>
              </n-button>
            </div>
          </section>

          <section class="config-section">
            <div class="section-heading">
              <span>{{ text.volumes }}</span>
              <n-button size="small" dashed @click="addVolume">
                <template #icon><n-icon><Plus /></n-icon></template>
                {{ text.addVolume }}
              </n-button>
            </div>
            <div v-for="(volume, index) in model.volumes" :key="index" class="mapping-row volume-row">
              <n-input v-model:value="volume.source" :placeholder="text.source" />
              <ArrowRight class="mapping-arrow" :size="15" />
              <n-input v-model:value="volume.target" :placeholder="text.target" />
              <label class="readonly-toggle">
                <n-switch v-model:value="volume.readOnly" size="small" />
                <span>{{ text.readOnly }}</span>
              </label>
              <n-button quaternary circle size="small" :aria-label="dict.common.delete" @click="model.volumes.splice(index, 1)">
                <template #icon><n-icon><Trash2 /></n-icon></template>
              </n-button>
            </div>
          </section>
        </n-tab-pane>

        <n-tab-pane name="runtime" :tab="text.runtime">
          <div class="form-grid runtime-grid">
            <n-form-item :label="text.restartPolicy" path="restartPolicy">
              <n-select
                :value="model.restartPolicy"
                :options="restartOptions"
                @update:value="onRestartPolicyChange"
              />
            </n-form-item>
            <n-form-item :label="text.network" path="network">
              <n-input v-model:value="model.network" :placeholder="text.networkPlaceholder" />
            </n-form-item>
          </div>
          <div class="runtime-option">
            <div>
              <strong>{{ text.autoRemove }}</strong>
              <small>{{ text.autoRemoveHint }}</small>
            </div>
            <n-switch v-model:value="model.autoRemove" :disabled="model.restartPolicy !== 'no'" />
          </div>
          <div class="runtime-option">
            <strong>{{ text.startAfterCreate }}</strong>
            <n-switch v-model:value="model.startAfterCreate" />
          </div>
        </n-tab-pane>
      </n-tabs>
    </n-form>
    <template #footer>
      <div class="modal-actions">
        <n-button :disabled="creating" @click="visible = false">{{ dict.common.cancel }}</n-button>
        <n-button type="primary" :loading="creating" @click="submit">
          {{ creating ? text.creating : text.submit }}
        </n-button>
      </div>
    </template>
  </n-modal>
</template>

<style scoped>
.form-grid { display: grid; align-items: start; grid-template-columns: 1fr 1fr; gap: 16px; }
.field-hint { display: block; width: 100%; margin-top: 6px; color: var(--sw-muted); font-size: 10px; line-height: 1.6; }
.image-field { width: 100%; }
.auto-empty { color: var(--sw-muted); font-size: 11px; }
.code-input :deep(textarea) { font-family: Consolas, monospace; font-size: 12px; }
.config-section + .config-section { margin-top: 25px; padding-top: 20px; border-top: 1px solid var(--sw-border); }
.section-heading { display: flex; align-items: center; justify-content: space-between; margin-bottom: 12px; color: var(--sw-text); font-size: 13px; font-weight: 600; }
.mapping-row { display: grid; align-items: center; gap: 9px; margin-bottom: 10px; }
.mapping-row :deep(.n-input-number) { width: 100%; }
.port-row { grid-template-columns: minmax(100px, 1fr) 16px minmax(100px, 1fr) 100px 32px; }
.volume-row { grid-template-columns: minmax(130px, 1fr) 16px minmax(130px, 1fr) 76px 32px; }
.mapping-arrow { color: var(--sw-muted); }
.readonly-toggle { display: flex; align-items: center; gap: 6px; color: var(--sw-text-secondary); font-size: 11px; }
.runtime-grid { margin-top: 5px; }
.runtime-option { display: flex; align-items: center; justify-content: space-between; min-height: 68px; padding: 16px; margin-bottom: 12px; border: 1px solid var(--sw-border); border-radius: 8px; background: var(--sw-surface-soft); color: var(--sw-text); font-size: 13px; }
.runtime-option strong { display: block; font-weight: 500; }
.runtime-option small { display: block; margin-top: 5px; color: var(--sw-muted); font-size: 11px; }
.modal-actions { display: flex; justify-content: flex-end; gap: 10px; }
@media (max-width: 760px) {
  .form-grid { grid-template-columns: 1fr; gap: 0; }
}
</style>
