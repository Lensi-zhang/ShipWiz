import { ref } from "vue";
import { defineStore } from "pinia";
import { createDiscreteApi } from "naive-ui";
import { containerApi } from "../api/container";
import { dict } from "../dict";
import type { ContainerInfo, CreateContainerInput } from "../types";

const { message } = createDiscreteApi(["message"]);

export type ContainerAction = "start" | "stop" | "restart" | "remove";

export const useContainerStore = defineStore("container", () => {
  const items = ref<ContainerInfo[]>([]);
  const loading = ref(false);
  const failed = ref(false);
  const actionKey = ref<string | null>(null);
  let requestSequence = 0;

  async function load(nodeId: string, quiet = false) {
    const sequence = ++requestSequence;
    if (!quiet) loading.value = true;
    try {
      const result = await containerApi.list(nodeId);
      if (sequence === requestSequence) {
        items.value = result;
        failed.value = false;
      }
      return true;
    } catch {
      if (sequence === requestSequence) {
        items.value = [];
        failed.value = true;
      }
      return false;
    } finally {
      if (sequence === requestSequence) loading.value = false;
    }
  }

  async function create(nodeId: string, input: CreateContainerInput) {
    try {
      const id = await containerApi.create(nodeId, input);
      message.success(dict.container.messages.created);
      await load(nodeId, true);
      return id;
    } catch {
      return null;
    }
  }

  async function act(action: ContainerAction, nodeId: string, containerId: string) {
    actionKey.value = `${action}:${containerId}`;
    try {
      await containerApi[action](nodeId, containerId);
      const messages = {
        start: dict.container.messages.started,
        stop: dict.container.messages.stopped,
        restart: dict.container.messages.restarted,
        remove: dict.container.messages.removed,
      };
      message.success(messages[action]);
      await load(nodeId, true);
      return true;
    } catch {
      return false;
    } finally {
      actionKey.value = null;
    }
  }

  return { items, loading, failed, actionKey, load, create, act };
});
