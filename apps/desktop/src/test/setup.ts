import { beforeEach } from "vite-plus/test";
import { useNodeListStore } from "@voya/client/node-list-store";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";

import "@voya/features/test/jest-dom";
import "@voya/features/test/setup-dom";

// The node list view persists across launches, and the runtime event store is
// module state: every test starts from neither.
beforeEach(() => {
  useNodeListStore.setState(useNodeListStore.getInitialState());
  useRuntimeEventStore.setState(useRuntimeEventStore.getInitialState());
});
