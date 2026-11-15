import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { containerApi } from "../api/container";
import type { ContainerInfo } from "../types";
import { useContainerStore } from "./container";

vi.mock("../api/container", () => ({
  containerApi: {
    list: vi.fn(),
    create: vi.fn(),
    start: vi.fn(),
    stop: vi.fn(),
    restart: vi.fn(),
    remove: vi.fn(),
  },
}));
vi.mock("../utils/message", () => ({
  anchorSuccess: vi.fn(),
  departSuccess: vi.fn(),
  hoistSuccess: vi.fn(),
  launchSuccess: vi.fn(),
  restartSuccess: vi.fn(),
}));

const listMock = vi.mocked(containerApi.list);

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

function container(id: string): ContainerInfo {
  return {
    id,
    shortId: id,
    name: id,
    image: "nginx:latest",
    imageId: "image-id",
    state: "running",
    status: "Up",
    created: 1,
    ports: [],
    cpuPercent: 0,
    memoryUsage: 0,
    memoryLimit: 0,
    statsAvailable: false,
  };
}

beforeEach(() => {
  setActivePinia(createPinia());
  listMock.mockReset();
});

describe("container store request races", () => {
  it("keeps loading and current items when an older request settles first", async () => {
    const store = useContainerStore();
    const existing = container("existing");
    const stale = container("stale");
    const fresh = container("fresh");
    const older = deferred<ContainerInfo[]>();
    const newer = deferred<ContainerInfo[]>();
    store.items = [existing];
    listMock
      .mockReturnValueOnce(older.promise)
      .mockReturnValueOnce(newer.promise);

    const olderLoad = store.load("old-node");
    const newerLoad = store.load("new-node");
    expect(store.loading).toBe(true);

    older.resolve([stale]);
    await expect(olderLoad).resolves.toBe(true);
    expect(store.items).toEqual([existing]);
    expect(store.loading).toBe(true);

    newer.resolve([fresh]);
    await expect(newerLoad).resolves.toBe(true);
    expect(store.items).toEqual([fresh]);
    expect(store.failed).toBe(false);
    expect(store.loading).toBe(false);
  });

  it("does not let a late older success overwrite newer results", async () => {
    const store = useContainerStore();
    const stale = container("stale");
    const fresh = container("fresh");
    const older = deferred<ContainerInfo[]>();
    const newer = deferred<ContainerInfo[]>();
    listMock
      .mockReturnValueOnce(older.promise)
      .mockReturnValueOnce(newer.promise);

    const olderLoad = store.load("old-node");
    const newerLoad = store.load("new-node");
    newer.resolve([fresh]);
    await expect(newerLoad).resolves.toBe(true);
    expect(store.items).toEqual([fresh]);
    expect(store.loading).toBe(false);

    older.resolve([stale]);
    await expect(olderLoad).resolves.toBe(true);
    expect(store.items).toEqual([fresh]);
    expect(store.failed).toBe(false);
    expect(store.loading).toBe(false);
  });

  it("ignores a stale failure after the newest request succeeds", async () => {
    const store = useContainerStore();
    const fresh = container("fresh");
    const older = deferred<ContainerInfo[]>();
    const newer = deferred<ContainerInfo[]>();
    listMock
      .mockReturnValueOnce(older.promise)
      .mockReturnValueOnce(newer.promise);

    const olderLoad = store.load("old-node");
    const newerLoad = store.load("new-node");
    newer.resolve([fresh]);
    await newerLoad;

    older.reject(new Error("stale failure"));
    await expect(olderLoad).resolves.toBe(false);
    expect(store.items).toEqual([fresh]);
    expect(store.failed).toBe(false);
    expect(store.loading).toBe(false);
  });
});
