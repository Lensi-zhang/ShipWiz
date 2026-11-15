<script setup lang="ts">
import { reactive, ref } from "vue";
import {
  NButton,
  NCheckbox,
  NEmpty,
  NForm,
  NFormItem,
  NIcon,
  NInput,
  NModal,
  NPopconfirm,
  NSpin,
  NTag,
} from "naive-ui";
import { alertWarning } from "../utils/message";
import { Anchor, Check, Pencil, Plus, Trash2 } from "lucide-vue-next";
import { dict } from "../dict";
import { useImageStore } from "../store/image";
import type { RegistrySource } from "../types";

const images = useImageStore();
const modalVisible = ref(false);
const saving = ref(false);
const form = reactive({
  id: null as string | null,
  name: "",
  url: "",
  isDefault: false,
});

function openCreate() {
  Object.assign(form, { id: null, name: "", url: "", isDefault: false });
  modalVisible.value = true;
}
function openEdit(registry: RegistrySource) {
  Object.assign(form, {
    id: registry.id,
    name: registry.name,
    url: registry.url,
    isDefault: registry.isDefault,
  });
  modalVisible.value = true;
}
async function save() {
  if (!form.name.trim() || !form.url.trim()) {
    alertWarning(dict.image.registry.required);
    return;
  }
  saving.value = true;
  const success = await images.saveRegistry(
    { name: form.name.trim(), url: form.url.trim(), isDefault: form.isDefault },
    form.id,
  );
  saving.value = false;
  if (success) modalVisible.value = false;
}
</script>

<template>
  <section class="registry-section">
    <div class="registry-toolbar">
      <div>
        <h2>{{ dict.image.tabs.registries }}</h2>
        <p>{{ dict.image.pullModal.hint }}</p>
      </div>
      <n-button type="primary" secondary @click="openCreate">
        <template #icon
          ><n-icon><Plus /></n-icon
        ></template>
        {{ dict.image.registry.add }}
      </n-button>
    </div>

    <n-spin :show="images.registriesLoading">
      <div v-if="images.registries.length" class="registry-grid">
        <article
          v-for="registry in images.registries"
          :key="registry.id"
          class="registry-card"
        >
          <div class="registry-icon"><Anchor :size="20" /></div>
          <div class="registry-copy">
            <div class="registry-name">
              <h3>{{ registry.name }}</h3>
              <n-tag
                v-if="registry.isDefault"
                size="small"
                type="success"
                :bordered="false"
              >
                {{ dict.image.registry.default }}
              </n-tag>
            </div>
            <code :title="registry.url">{{ registry.url }}</code>
            <span v-if="registry.id === 'docker-hub'">{{
              dict.image.registry.officialHint
            }}</span>
          </div>
          <div class="registry-actions">
            <n-button
              v-if="!registry.isDefault"
              size="small"
              quaternary
              @click="images.setDefaultRegistry(registry.id)"
            >
              <template #icon
                ><n-icon><Check /></n-icon
              ></template>
              {{ dict.image.registry.setDefault }}
            </n-button>
            <n-button
              v-if="registry.id !== 'docker-hub'"
              quaternary
              circle
              size="small"
              :aria-label="dict.common.edit"
              @click="openEdit(registry)"
            >
              <template #icon
                ><n-icon><Pencil /></n-icon
              ></template>
            </n-button>
            <n-popconfirm
              v-if="registry.id !== 'docker-hub'"
              :positive-text="dict.common.delete"
              :negative-text="dict.common.cancel"
              @positive-click="images.removeRegistry(registry.id)"
            >
              <template #trigger>
                <n-button
                  quaternary
                  circle
                  size="small"
                  type="error"
                  :aria-label="dict.common.delete"
                >
                  <template #icon
                    ><n-icon><Trash2 /></n-icon
                  ></template>
                </n-button>
              </template>
              <strong>{{ dict.image.registry.removeTitle }}</strong>
              <p class="registry-confirm">
                {{ dict.image.registry.removeDescription }}
              </p>
            </n-popconfirm>
          </div>
        </article>
      </div>
      <n-empty
        v-else-if="images.registriesFailed"
        description="补给港加载失败，请检查配置后重试。"
        class="registry-empty"
      >
        <template #extra
          ><n-button size="small" @click="images.loadRegistries()">{{
            dict.common.retry
          }}</n-button></template
        >
      </n-empty>
      <n-empty
        v-else-if="!images.registriesLoading"
        :description="dict.image.registry.empty"
        class="registry-empty"
      />
    </n-spin>

    <n-modal
      v-model:show="modalVisible"
      preset="card"
      :title="form.id ? dict.image.registry.edit : dict.image.registry.add"
      class="form-modal registry-modal"
    >
      <n-form label-placement="top" @submit.prevent="save">
        <n-form-item :label="dict.image.registry.name" required>
          <n-input
            v-model:value="form.name"
            :placeholder="dict.image.registry.namePlaceholder"
          />
        </n-form-item>
        <n-form-item :label="dict.image.registry.url" required>
          <n-input
            v-model:value="form.url"
            :placeholder="dict.image.registry.urlPlaceholder"
          />
        </n-form-item>
        <n-checkbox v-model:checked="form.isDefault">{{
          dict.image.registry.makeDefault
        }}</n-checkbox>
        <div class="modal-actions">
          <n-button @click="modalVisible = false">{{
            dict.common.cancel
          }}</n-button>
          <n-button type="primary" :loading="saving" @click="save">{{
            dict.common.save
          }}</n-button>
        </div>
      </n-form>
    </n-modal>
  </section>
</template>

<style scoped>
.registry-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 16px;
  border-bottom: 1px solid var(--sw-border);
}
.registry-toolbar h2 {
  margin: 0;
  color: var(--sw-text);
  font-size: 15px;
  font-weight: 500;
}
.registry-toolbar p {
  margin: 5px 0 0;
  color: var(--sw-muted);
  font-size: 10px;
}
.registry-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 13px;
  padding: 16px;
}
.registry-card {
  display: flex;
  align-items: center;
  gap: 13px;
  min-width: 0;
  padding: 16px;
  border: 1px solid var(--sw-border);
  border-radius: 8px;
  background: var(--sw-surface);
}
.registry-icon {
  display: grid;
  place-items: center;
  flex: 0 0 auto;
  width: 38px;
  height: 38px;
  color: var(--sw-primary);
  border-radius: 8px;
  background: var(--sw-primary-soft);
}
.registry-copy {
  flex: 1;
  min-width: 0;
}
.registry-name {
  display: flex;
  align-items: center;
  gap: 8px;
}
.registry-name h3 {
  overflow: hidden;
  margin: 0;
  color: var(--sw-text);
  font-size: 13px;
  font-weight: 500;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.registry-copy code {
  display: block;
  overflow: hidden;
  margin-top: 6px;
  color: var(--sw-text-secondary);
  font-size: 10px;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.registry-copy > span {
  display: block;
  margin-top: 5px;
  color: var(--sw-muted);
  font-size: 9px;
}
.registry-actions {
  display: flex;
  align-items: center;
  flex: 0 0 auto;
  gap: 2px;
}
.registry-confirm {
  max-width: 250px;
  margin: 6px 0 0;
  line-height: 1.6;
}
.registry-empty {
  min-height: 260px;
  justify-content: center;
}
.registry-modal {
  width: 500px;
  max-width: calc(100vw - 40px);
}
.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  margin-top: 24px;
}
@media (max-width: 1000px) {
  .registry-grid {
    grid-template-columns: 1fr;
  }
}
</style>
