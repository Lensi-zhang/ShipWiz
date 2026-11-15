import { getVersion } from "@tauri-apps/api/app";
import { flushPromises, shallowMount } from "@vue/test-utils";
import { describe, expect, it, vi } from "vitest";
import { version as packageVersion } from "../../../package.json";
import Setting from "./index.vue";

vi.mock("@tauri-apps/api/app", () => ({ getVersion: vi.fn() }));
vi.mock("@tauri-apps/plugin-updater", () => ({ check: vi.fn() }));
vi.mock("@tauri-apps/plugin-process", () => ({ relaunch: vi.fn() }));
vi.mock("../../api/update", () => ({
  checkUpdate: vi.fn(),
  clearSkippedUpdate: vi.fn(),
  getSkippedUpdate: vi.fn().mockResolvedValue(null),
  skipUpdate: vi.fn(),
}));
vi.mock("../../store/fleet", () => ({
  useFleetStore: () => ({
    state: { nodes: [], currentNodeId: null },
    currentNode: null,
    load: vi.fn().mockResolvedValue(true),
    setCurrent: vi.fn(),
    test: vi.fn(),
  }),
}));
vi.mock("../../store/appearance", () => ({
  useAppearanceStore: () => ({
    theme: "deep",
    collapsed: false,
    logTail: 100,
    logFollow: true,
    setTheme: vi.fn(),
  }),
}));
vi.mock("../../store/image", () => ({
  useImageStore: () => ({ loadRegistries: vi.fn().mockResolvedValue(true) }),
}));
vi.mock("../../utils/message", () => ({
  alertError: vi.fn(),
  alertWarning: vi.fn(),
  connectSuccess: vi.fn(),
  message: { success: vi.fn() },
}));

describe("Setting version fallback", () => {
  it("uses the package version when the Tauri application API is unavailable", async () => {
    vi.mocked(getVersion).mockRejectedValueOnce(new Error("Browser context"));

    const wrapper = shallowMount(Setting, {
      global: {
        renderStubDefaultSlot: true,
        stubs: {
          "router-link": { template: "<a><slot /></a>" },
        },
      },
    });
    await flushPromises();

    expect(wrapper.text()).toContain(packageVersion);
    expect(wrapper.text()).not.toContain("0.1.0");
  });
});
