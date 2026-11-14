import { command } from "./tauri";
import type {
  ImageInfo,
  ImageOperationResult,
  ImageSearchResult,
  RegistryInput,
  RegistrySource,
} from "../types";

export const imageApi = {
  list: (nodeId: string) => command<ImageInfo[]>("list_images", { nodeId }),
  search: (nodeId: string, term: string) =>
    command<ImageSearchResult[]>("search_images", { nodeId, term }),
  pull: (nodeId: string, reference: string, registryId: string | null) =>
    command<ImageOperationResult>("pull_image", { nodeId, reference, registryId }),
  remove: (nodeId: string, imageId: string, force = false) =>
    command<boolean>("remove_image", { nodeId, imageId, force }),
  exportImage: (nodeId: string, reference: string, destination: string) =>
    command<boolean>("export_image", { nodeId, reference, destination }),
  importImage: (nodeId: string, source: string) =>
    command<ImageOperationResult>("import_image", { nodeId, source }),
  registries: () => command<RegistrySource[]>("list_registries"),
  saveRegistry: (input: RegistryInput, id: string | null) =>
    command<RegistrySource[]>("save_registry", { input, id }),
  removeRegistry: (id: string) => command<RegistrySource[]>("remove_registry", { id }),
  setDefaultRegistry: (id: string) =>
    command<RegistrySource[]>("set_default_registry", { id }),
};
