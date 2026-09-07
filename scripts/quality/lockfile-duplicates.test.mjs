import { describe, expect, it } from "vitest";

import { checkLockfile, resolvedVersions } from "./lockfile-duplicates.mjs";

const lockfile = [
  "packages:",
  "",
  "  '@radix-ui/react-dialog@1.1.15':",
  "    resolution: {integrity: sha512-aaa}",
  "  '@radix-ui/react-dialog@1.1.17':",
  "    resolution: {integrity: sha512-bbb}",
  "  '@radix-ui/react-slot@1.3.0':",
  "    resolution: {integrity: sha512-ccc}",
  "  react@19.2.6:",
  "    resolution: {integrity: sha512-ddd}",
  "",
  "snapshots:",
  "",
  "  '@radix-ui/react-dialog@1.1.17(@types/react@19.2.15)(react@19.2.6)':",
  "    dependencies:",
  "      '@radix-ui/react-slot': 1.3.0(react@19.2.6)",
].join("\n");

describe("lockfile duplicate gate", () => {
  it("collapses the peer-suffixed snapshot keys onto the bare version", () => {
    // `name@version(peer)(peer)` and `name@version` are the same install;
    // counting them separately would report a duplicate on every package.
    expect(resolvedVersions(lockfile, ["@radix-ui/react-slot"], [])).toEqual(
      new Map([["@radix-ui/react-slot", new Set(["1.3.0"])]]),
    );
  });

  it("fails when a package resolves to more than one version", () => {
    const { failures } = checkLockfile(lockfile, ["@radix-ui/react-dialog"], []);

    expect(failures).toEqual([
      "@radix-ui/react-dialog resolves to 2 versions: 1.1.15, 1.1.17",
    ]);
  });

  it("passes an unscoped package that resolves once", () => {
    const { failures, report } = checkLockfile(lockfile, ["react"], []);

    expect(failures).toEqual([]);
    expect(report).toEqual(["react: 19.2.6"]);
  });

  it("covers a whole scope without naming each package", () => {
    // The copies that broke the app were transitive ones nobody had listed.
    const { failures } = checkLockfile(lockfile, [], ["@radix-ui/"]);

    expect(failures).toEqual([
      "@radix-ui/react-dialog resolves to 2 versions: 1.1.15, 1.1.17",
    ]);
  });

  it("fails loudly when a listed package left the lockfile", () => {
    const { failures } = checkLockfile(lockfile, ["preact"], []);

    expect(failures).toEqual([
      "preact is not in the lockfile; drop it from the list or restore the dependency",
    ]);
  });
});
