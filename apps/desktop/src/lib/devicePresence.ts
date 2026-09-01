import type { DeviceView } from "../api/types";

export const DEVICE_OFFLINE_DEBOUNCE_MS = 2_000;

type PendingOffline = {
  timerId: number;
};

export class DeviceOfflineDebouncer {
  private displayedDevices: DeviceView[] = [];
  private latestDevices: DeviceView[] = [];
  private readonly pendingOffline = new Map<string, PendingOffline>();

  constructor(
    private readonly onChange: (devices: DeviceView[]) => void,
    private readonly delayMs = DEVICE_OFFLINE_DEBOUNCE_MS,
  ) {}

  initialize(devices: DeviceView[]) {
    this.cancelAll();
    this.displayedDevices = devices;
    this.latestDevices = devices;
  }

  update(devices: DeviceView[]) {
    this.latestDevices = devices;
    const incomingIds = new Set(devices.map((device) => device.id));
    const displayedById = new Map(
      this.displayedDevices.map((device) => [device.id, device]),
    );

    for (const id of this.pendingOffline.keys()) {
      if (!incomingIds.has(id)) this.cancel(id);
    }

    for (const device of devices) {
      const displayed = displayedById.get(device.id);
      if (
        device.connectionState === "offline" &&
        displayed?.connectionState === "online"
      ) {
        this.schedule(device.id);
      } else {
        this.cancel(device.id);
      }
    }

    this.emitLatest();
  }

  dispose() {
    this.cancelAll();
  }

  private schedule(id: string) {
    if (this.pendingOffline.has(id)) return;
    const timerId = window.setTimeout(() => {
      this.pendingOffline.delete(id);
      this.emitLatest();
    }, this.delayMs);
    this.pendingOffline.set(id, { timerId });
  }

  private cancel(id: string) {
    const pending = this.pendingOffline.get(id);
    if (!pending) return;
    window.clearTimeout(pending.timerId);
    this.pendingOffline.delete(id);
  }

  private cancelAll() {
    for (const pending of this.pendingOffline.values()) {
      window.clearTimeout(pending.timerId);
    }
    this.pendingOffline.clear();
  }

  private emitLatest() {
    const displayedById = new Map(
      this.displayedDevices.map((device) => [device.id, device]),
    );
    const visibleById = new Map(
      this.latestDevices.map((device) => [
        device.id,
        this.pendingOffline.has(device.id)
          ? { ...device, connectionState: "online" as const }
          : device,
      ]),
    );
    const nextDevices = this.pendingOffline.size
      ? [
          ...this.displayedDevices
            .map((device) => visibleById.get(device.id))
            .filter((device): device is DeviceView => Boolean(device)),
          ...this.latestDevices
            .filter((device) => !displayedById.has(device.id))
            .map((device) => visibleById.get(device.id)!),
        ]
      : this.latestDevices;
    this.displayedDevices = nextDevices;
    this.onChange(nextDevices);
  }
}
