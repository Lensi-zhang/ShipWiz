import { computed, ref } from "vue";
import { defineStore } from "pinia";
import { departSuccess, hoistSuccess, steerSuccess } from "../utils/message";
import { listen } from "@tauri-apps/api/event";
import { imageApi } from "../api/image";
import { dict } from "../dict";
import type {
  ImageInfo,
  ImagePullProgress,
  ImagePullProgressEvent,
  ImageSearchResult,
  RegistryInput,
  RegistrySource,
} from "../types";

export const useImageStore = defineStore("image", () => {
  const items = ref<ImageInfo[]>([]);
  const registries = ref<RegistrySource[]>([]);
  const registriesLoading = ref(false);
  const registriesFailed = ref(false);
  const searchResults = ref<ImageSearchResult[]>([]);
  const loading = ref(false);
  const searching = ref(false);
  const failed = ref(false);
  const actionKey = ref<string | null>(null);
  const pullOperationId = ref<string | null>(null);
  const pullProgress = ref<Record<string, ImagePullProgress>>({});
  const pullStatus = ref("");
  let requestSequence = 0;
  let pullListenerReady: Promise<void> | null = null;

  const pullLayers = computed(() => Object.values(pullProgress.value));
  const pullCurrent = computed(() =>
    pullLayers.value.reduce(
      (sum, layer) =>
        sum + (layer.total && layer.total > 0 ? (layer.current ?? 0) : 0),
      0,
    ),
  );
  const pullTotal = computed(() =>
    pullLayers.value.reduce(
      (sum, layer) => sum + (layer.total && layer.total > 0 ? layer.total : 0),
      0,
    ),
  );
  const pullPercent = computed(() =>
    pullTotal.value > 0
      ? Math.min(100, Math.round((pullCurrent.value / pullTotal.value) * 100))
      : null,
  );
  const totalSize = computed(() =>
    items.value.reduce((total, image) => total + image.size, 0),
  );
  const inUseCount = computed(
    () => items.value.filter((image) => image.containers > 0).length,
  );

  async function load(nodeId: string) {
    const sequence = ++requestSequence;
    loading.value = true;
    failed.value = false;
    try {
      const result = await imageApi.list(nodeId);
      if (sequence === requestSequence) items.value = result;
      return true;
    } catch {
      if (sequence === requestSequence) failed.value = true;
      return false;
    } finally {
      if (sequence === requestSequence) loading.value = false;
    }
  }

  async function search(nodeId: string, term: string) {
    if (!term.trim()) return false;
    searching.value = true;
    try {
      searchResults.value = await imageApi.search(nodeId, term.trim());
      return true;
    } catch {
      return false;
    } finally {
      searching.value = false;
    }
  }

  function ensurePullListener() {
    if (!pullListenerReady) {
      pullListenerReady = listen<ImagePullProgressEvent>(
        "shipwiz:image-pull-progress",
        (event) => {
          if (event.payload.operationId !== pullOperationId.value) return;
          const progress = event.payload.progress;
          pullStatus.value = progress.status;
          if (progress.layerId) {
            pullProgress.value[progress.layerId] = progress;
          }
        },
      )
        .then(() => undefined)
        .catch(() => {
          pullListenerReady = null;
        });
    }
    return pullListenerReady;
  }

  async function pull(
    nodeId: string,
    reference: string,
    registryId: string | null,
  ) {
    if (pullOperationId.value) return false;
    const operationId = crypto.randomUUID();
    pullOperationId.value = operationId;
    pullProgress.value = {};
    pullStatus.value = "正在连接 Docker…";
    actionKey.value = `pull:${reference}`;
    try {
      await ensurePullListener();
      await imageApi.pull(nodeId, reference, registryId, operationId);
      pullStatus.value = "拉取完成";
      hoistSuccess(dict.image.messages.pulled);
      await load(nodeId);
      return true;
    } catch {
      pullStatus.value = "拉取失败";
      return false;
    } finally {
      if (pullOperationId.value === operationId) pullOperationId.value = null;
      actionKey.value = null;
    }
  }

  async function remove(nodeId: string, image: ImageInfo) {
    actionKey.value = `remove:${image.id}`;
    try {
      await imageApi.remove(nodeId, image.id);
      departSuccess(dict.image.messages.removed);
      await load(nodeId);
      return true;
    } catch {
      return false;
    } finally {
      actionKey.value = null;
    }
  }

  async function importImage(nodeId: string, source: string) {
    actionKey.value = "import";
    try {
      await imageApi.importImage(nodeId, source);
      hoistSuccess(dict.image.messages.imported);
      await load(nodeId);
      return true;
    } catch {
      return false;
    } finally {
      actionKey.value = null;
    }
  }

  async function exportImage(
    nodeId: string,
    reference: string,
    destination: string,
  ) {
    actionKey.value = `export:${reference}`;
    try {
      await imageApi.exportImage(nodeId, reference, destination);
      departSuccess(dict.image.messages.exported);
      return true;
    } catch {
      return false;
    } finally {
      actionKey.value = null;
    }
  }

  async function loadRegistries() {
    registriesLoading.value = true;
    registriesFailed.value = false;
    try {
      registries.value = await imageApi.registries();
      return true;
    } catch {
      registriesFailed.value = true;
      return false;
    } finally {
      registriesLoading.value = false;
    }
  }

  async function saveRegistry(input: RegistryInput, id: string | null) {
    actionKey.value = "registry";
    try {
      registries.value = await imageApi.saveRegistry(input, id);
      steerSuccess(
        id
          ? dict.image.messages.registryUpdated
          : dict.image.messages.registryCreated,
      );
      return true;
    } catch {
      return false;
    } finally {
      actionKey.value = null;
    }
  }

  async function removeRegistry(id: string) {
    actionKey.value = `registry:${id}`;
    try {
      registries.value = await imageApi.removeRegistry(id);
      departSuccess(dict.image.messages.registryRemoved);
      return true;
    } catch {
      return false;
    } finally {
      actionKey.value = null;
    }
  }

  async function setDefaultRegistry(id: string) {
    try {
      registries.value = await imageApi.setDefaultRegistry(id);
      steerSuccess(dict.image.messages.registryDefaulted);
      return true;
    } catch {
      return false;
    }
  }

  return {
    items,
    registries,
    registriesLoading,
    registriesFailed,
    searchResults,
    loading,
    searching,
    failed,
    actionKey,
    pullStatus,
    pullLayers,
    pullCurrent,
    pullTotal,
    pullPercent,
    totalSize,
    inUseCount,
    load,
    search,
    pull,
    remove,
    importImage,
    exportImage,
    loadRegistries,
    saveRegistry,
    removeRegistry,
    setDefaultRegistry,
  };
});
