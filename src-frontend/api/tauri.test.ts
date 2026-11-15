import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { dict } from "../dict";
import { alertError } from "../utils/message";
import { command } from "./tauri";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("../utils/message", () => ({ alertError: vi.fn() }));

const invokeMock = vi.mocked(invoke);
const alertErrorMock = vi.mocked(alertError);

beforeEach(() => {
  invokeMock.mockReset();
  alertErrorMock.mockReset();
});

describe("command", () => {
  it("rethrows IPC rejections after showing the original error", async () => {
    const failure = new Error("IPC unavailable");
    invokeMock.mockRejectedValueOnce(failure);

    await expect(command("list_containers")).rejects.toBe(failure);
    expect(alertErrorMock).toHaveBeenCalledWith("IPC unavailable", {
      duration: 6000,
      closable: true,
    });
  });

  it("rejects non-zero business responses instead of returning their data", async () => {
    invokeMock.mockResolvedValueOnce({
      code: 500,
      msg: "Docker unavailable",
      data: [],
    });

    await expect(command("list_containers")).rejects.toThrow(
      "Docker unavailable",
    );
    expect(alertErrorMock).toHaveBeenCalledWith("Docker unavailable", {
      duration: 6000,
      closable: true,
    });
  });

  it.each([false, 0, ""])(
    "preserves the valid falsy response %j",
    async (data) => {
      invokeMock.mockResolvedValueOnce({ code: 0, msg: "", data });

      await expect(command<typeof data>("successful_command")).resolves.toBe(
        data,
      );
      expect(alertErrorMock).not.toHaveBeenCalled();
    },
  );

  it("rejects null success payloads as empty data", async () => {
    invokeMock.mockResolvedValueOnce({ code: 0, msg: "", data: null });

    await expect(command("successful_command")).rejects.toThrow(
      dict.errors.emptyData,
    );
    expect(alertErrorMock).toHaveBeenCalledWith(dict.errors.emptyData, {
      duration: 6000,
      closable: true,
    });
  });
});
