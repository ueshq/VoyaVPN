import { beforeEach } from "vitest";
import { useNodeListStore } from "@voya/client/node-list-store";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";

import "@testing-library/jest-dom/vitest";
import "@voya/features/test/setup-dom";

// The node list view persists across launches, and the runtime event store is
// module state: every test starts from neither.
beforeEach(() => {
  useNodeListStore.setState(useNodeListStore.getInitialState());
  useRuntimeEventStore.setState(useRuntimeEventStore.getInitialState());
});
