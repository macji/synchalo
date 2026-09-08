import { describe, expect, it } from "vitest";

import type { DeviceView } from "../api/types";
import { formatDeviceSystem } from "./format";

describe("device system address", () => {
  it.each([
    ["192.168.1.12:53317", "macOS · 192.168.1.12"],
    ["[fd00::12]:53317", "macOS · fd00::12"],
    ["fd00::12", "macOS · fd00::12"],
    [null, "macOS"],
  ])("formats %s without a transport port", (address, expected) => {
    expect(formatDeviceSystem({ platform: "macos", address } as DeviceView, "未知系统")).toBe(expected);
  });
});
