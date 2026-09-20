import { beforeEach } from "vitest";
import { useNodeListStore } from "@voya/client/node-list-store";

import "@testing-library/jest-dom/vitest";

class ResizeObserverMock {
  disconnect() {}
  observe() {}
  unobserve() {}
}

if (!globalThis.ResizeObserver) {
  globalThis.ResizeObserver = ResizeObserverMock as typeof ResizeObserver;
}

if (!globalThis.PointerEvent) {
  globalThis.PointerEvent = MouseEvent as typeof PointerEvent;
}

if (!Element.prototype.hasPointerCapture) {
  Element.prototype.hasPointerCapture = () => false;
}

if (!Element.prototype.setPointerCapture) {
  Element.prototype.setPointerCapture = () => {};
}

if (!Element.prototype.releasePointerCapture) {
  Element.prototype.releasePointerCapture = () => {};
}

if (!Element.prototype.scrollIntoView) {
  Element.prototype.scrollIntoView = () => {};
}


// The node list view persists across launches; every test starts from none.
beforeEach(() => {
  useNodeListStore.setState(useNodeListStore.getInitialState());
});

/**
 * The same backend registration `platform-boot` does at startup, so shared
 * code in `@voya/client` reaches the desktop binding.
 *
 * Imported here rather than at the top of the file, and per test rather than
 * once: a test file's `vi.mock("@/ipc/commands")` is registered only after the
 * setup files have run, so an eager import would hand the shared seam the real
 * Tauri binding and every mocked command would be bypassed.
 */
beforeEach(async () => {
  const { registerDesktopBackend } = await import("@/ipc/register-backend");
  registerDesktopBackend();
});
