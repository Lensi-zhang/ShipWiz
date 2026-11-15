import { ref } from "vue";
import { defineStore } from "pinia";
import { anchorSuccess, departSuccess, hoistSuccess, launchSuccess, restartSuccess } from "../utils/message";
import { containerApi } from "../api/container";
import { dict } from "../dict";
import type { ContainerInfo, CreateContainerInput } from "../types";


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
      hoistSuccess(dict.container.messages.created);
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
      if (action === "start") launchSuccess(messages[action]);
      else if (action === "stop") anchorSuccess(messages[action]);
      else if (action === "restart") restartSuccess(messages[action]);
      else departSuccess(messages[action]);
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
