import { beforeEach } from "vitest";
import { useNodeListStore } from "@voya/client/node-list-store";

import "@testing-library/jest-dom/vitest";
import "@voya/features/test/setup-dom";

// The node list view persists across launches; every test starts from none.
beforeEach(() => {
  useNodeListStore.setState(useNodeListStore.getInitialState());
});
