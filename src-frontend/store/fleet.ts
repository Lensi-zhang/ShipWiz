import { computed, ref } from "vue";
import { defineStore } from "pinia";
import { containerApi } from "../api/container";
import { fleetApi } from "../api/fleet";
import type { FleetOverview, FleetState, NodeInput } from "../types";

export const useFleetStore = defineStore("fleet", () => {
  const state = ref<FleetState>({ nodes: [], currentNodeId: null });
  const loading = ref(false);
  const failed = ref(false);
  const testingId = ref<string | null>(null);
  const overview = ref<FleetOverview>({
    containers: 0,
    running: 0,
    images: 0,
    unavailable: 0,
  });
  const overviewLoading = ref(false);
  const currentNode = computed(
    () =>
      state.value.nodes.find((node) => node.id === state.value.currentNodeId) ??
      null,
  );
  const onlineCount = computed(
    () => state.value.nodes.filter((node) => node.status === "online").length,
  );

  async function load() {
    loading.value = true;
    failed.value = false;
    try {
      state.value = await fleetApi.getState();
      return true;
    } catch {
      failed.value = true;
      return false;
    } finally {
      loading.value = false;
    }
  }

  async function loadOverview() {
    overviewLoading.value = true;
    try {
      overview.value = await containerApi.overview();
    } catch {
      // The command wrapper presents connection and configuration errors.
    } finally {
      overviewLoading.value = false;
    }
  }

  async function save(input: NodeInput, id?: string) {
    try {
      state.value = id
        ? await fleetApi.update(id, input)
        : await fleetApi.create(input);
      return true;
    } catch {
      return false;
    }
  }

  async function remove(id: string) {
    try {
      state.value = await fleetApi.remove(id);
      return true;
    } catch {
      return false;
    }
  }

  async function setCurrent(id: string) {
    try {
      state.value = await fleetApi.setCurrent(id);
      return true;
    } catch {
      return false;
    }
  }

  async function test(id: string) {
    testingId.value = id;
    try {
      const updated = await fleetApi.test(id);
      const index = state.value.nodes.findIndex((node) => node.id === id);
      if (index >= 0) state.value.nodes[index] = updated;
      return true;
    } catch {
      await load();
      return false;
    } finally {
      testingId.value = null;
    }
  }

  return {
    state,
    loading,
    failed,
    testingId,
    overview,
    overviewLoading,
    currentNode,
    onlineCount,
    load,
    loadOverview,
    save,
    remove,
    setCurrent,
    test,
  };
});
