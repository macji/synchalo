import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { DeviceConnectionState, DeviceView } from "../api/types";
import { DEVICE_OFFLINE_DEBOUNCE_MS, DeviceOfflineDebouncer } from "./devicePresence";

describe("DeviceOfflineDebouncer", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("hides a transient offline transition", () => {
    const updates: DeviceView[][] = [];
    const debouncer = new DeviceOfflineDebouncer((devices) => updates.push(devices));
    debouncer.initialize([device("a", "online")]);

    debouncer.update([device("a", "offline")]);
    expect(lastState(updates, "a")).toBe("online");

    vi.advanceTimersByTime(750);
    debouncer.update([device("a", "online")]);
    vi.advanceTimersByTime(DEVICE_OFFLINE_DEBOUNCE_MS);

    expect(lastState(updates, "a")).toBe("online");
    expect(updates.every((devices) => stateOf(devices, "a") === "online")).toBe(true);
  });

  it("shows an offline device after the debounce window", () => {
    const updates: DeviceView[][] = [];
    const debouncer = new DeviceOfflineDebouncer((devices) => updates.push(devices));
    debouncer.initialize([device("a", "online"), device("b", "online")]);

    debouncer.update([device("b", "online"), device("a", "offline")]);
    expect(updates.at(-1)?.map((entry) => entry.id)).toEqual(["a", "b"]);
    expect(lastState(updates, "a")).toBe("online");

    vi.advanceTimersByTime(DEVICE_OFFLINE_DEBOUNCE_MS - 1);
    expect(lastState(updates, "a")).toBe("online");
    vi.advanceTimersByTime(1);

    expect(lastState(updates, "a")).toBe("offline");
    expect(updates.at(-1)?.map((entry) => entry.id)).toEqual(["b", "a"]);
  });

  it("does not restart the timer for repeated offline events", () => {
    const updates: DeviceView[][] = [];
    const debouncer = new DeviceOfflineDebouncer((devices) => updates.push(devices));
    debouncer.initialize([device("a", "online")]);

    debouncer.update([device("a", "offline")]);
    vi.advanceTimersByTime(1_500);
    debouncer.update([device("a", "offline")]);
    vi.advanceTimersByTime(500);

    expect(lastState(updates, "a")).toBe("offline");
  });
});

function device(id: string, connectionState: DeviceConnectionState): DeviceView {
  return {
    id,
    name: `Device ${id}`,
    platform: "linux",
    connectionState,
    isCurrent: false,
    address: null,
    lastSeenAt: null,
    lastSyncAt: null,
    paused: false,
  };
}

function lastState(updates: DeviceView[][], id: string): DeviceConnectionState | undefined {
  return stateOf(updates.at(-1) ?? [], id);
}

function stateOf(devices: DeviceView[], id: string): DeviceConnectionState | undefined {
  return devices.find((device) => device.id === id)?.connectionState;
}
