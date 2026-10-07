import { mkdirSync, writeFileSync } from "node:fs";
import { mkdtemp, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vite-plus/test";
import { findUniversalMacosFramework, stageLibbox } from "./build-libbox.mjs";

const temporaryDirectories = [];

afterEach(async () => {
  const { rm } = await import("node:fs/promises");
  await Promise.all(temporaryDirectories.splice(0).map((path) => rm(path, { force: true, recursive: true })));
});

async function fixture() {
  const root = await mkdtemp(join(tmpdir(), "voyavpn-libbox-"));
  temporaryDirectories.push(root);
  const xcframework = join(root, "Libbox.xcframework");
  const framework = join(xcframework, "macos-arm64_x86_64", "Libbox.framework");
  mkdirSync(framework, { recursive: true });
  writeFileSync(join(framework, "Libbox"), "universal-macos");
  mkdirSync(join(xcframework, "ios-arm64", "Libbox.framework"), { recursive: true });
  writeFileSync(join(xcframework, "ios-arm64", "Libbox.framework", "Libbox"), "ios");
  return { root, xcframework, framework };
}

describe("Libbox macOS staging", () => {
  it("selects the universal macOS framework instead of iOS slices", async () => {
    const { xcframework, framework } = await fixture();
    expect(findUniversalMacosFramework(xcframework)).toBe(framework);
  });

  it("stages only Libbox.framework and discards the generated XCFramework", async () => {
    const { root, xcframework } = await fixture();
    const destination = join(root, "Frameworks", "Libbox.framework");
    stageLibbox({
      outputXCFramework: xcframework,
      destinationFramework: destination,
      captureCommand: () => ({ status: 0, stdout: "arm64 x86_64", stderr: "" }),
    });

    expect(await readFile(join(destination, "Libbox"), "utf8")).toBe("universal-macos");
    await expect(readFile(join(xcframework, "ios-arm64", "Libbox.framework", "Libbox"))).rejects.toThrow();
  });

  it("rejects a single-architecture framework", async () => {
    const { root, xcframework } = await fixture();
    expect(() => stageLibbox({
      outputXCFramework: xcframework,
      destinationFramework: join(root, "Libbox.framework"),
      captureCommand: () => ({ status: 0, stdout: "arm64", stderr: "" }),
    })).toThrow(/missing x86_64/);
  });
});
