import "@testing-library/jest-dom/vitest";

import type { TestingLibraryMatchers } from "@testing-library/jest-dom/matchers";

// jest-dom's own `vitest` entry merges its matchers into `Assertion<T>`, the
// Vitest 4 shape. Vitest 5's `Assertion<R, T>` takes the return type first and
// merges custom matchers through `Matchers<R, T>`, so that merge never reaches
// `expect(...)`. The augmentation targets `vite-plus/test` rather than
// `vitest`: a bare `vitest` resolves to whichever pnpm peer variant sits next
// to the importing package, which need not be the one the tests import. Drop
// this when jest-dom ships Vitest 5 types.
declare module "vite-plus/test" {
  // The body is empty because the matchers come entirely from `extends`, and
  // the parameter list must match Vitest's declaration to merge.
  /* oxlint-disable no-empty-object-type */
  interface Matchers<R extends void | Promise<void> = void | Promise<void>, T = unknown> extends TestingLibraryMatchers<
    unknown,
    R
  > {}
  /* oxlint-enable no-empty-object-type */
}
