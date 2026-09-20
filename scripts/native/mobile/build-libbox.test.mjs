import { mkdirSync, writeFileSync } from "node:fs";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";

import { iosSlices, stageLibboxXCFramework } from "./build-libbox-ios.mjs";
import { stageLibboxAar } from "./build-libbox-android.mjs";

const temporaryDirectories = [];

afterEach(async () => {
  await Promise.all(temporaryDirectories.splice(0).map((path) => rm(path, { force: true, recursive: true })));
});

async function temporaryRoot(prefix) {
  const root = await mkdtemp(join(tmpdir(), prefix));
  temporaryDirectories.push(root);

  return root;
}

/** What `make lib_apple` leaves behind: every platform sing-box can build for. */
async function xcframework() {
  const root = await temporaryRoot("voyavpn-libbox-ios-");
  const path = join(root, "Libbox.xcframework");
  const slices = ["macos-arm64_x86_64", "ios-arm64", "ios-arm64_x86_64-simulator", "tvos-arm64"];
  for (const slice of slices) {
    mkdirSync(join(path, slice, "Libbox.framework"), { recursive: true });
    writeFileSync(join(path, slice, "Libbox.framework", "Libbox"), slice);
  }
  writeFileSync(join(path, "Info.plist"), "<plist/>");

  return { root, path, slices };
}

/** A `plutil` that answers from the slice list rather than reading a plist. */
function fakePlist(slices) {
  const removed = [];

  return {
    removed,
    read: (_path, keyPath) =>
      keyPath === "AvailableLibraries"
        ? String(slices.length)
        : slices[Number(keyPath.match(/AvailableLibraries\.(\d+)\./)?.[1] ?? -1)] ?? "",
    remove: (_path, keyPath) => removed.push(keyPath),
  };
}

describe("Libbox iOS staging", () => {
  it("keeps one device slice and one simulator slice, device first", async () => {
    const { path } = await xcframework();

    expect(iosSlices(path)).toEqual(["ios-arm64", "ios-arm64_x86_64-simulator"]);
  });

  it("rejects an xcframework that cannot serve both a device and the simulator", async () => {
    const root = await temporaryRoot("voyavpn-libbox-ios-partial-");
    const path = join(root, "Libbox.xcframework");
    mkdirSync(join(path, "ios-arm64"), { recursive: true });

    expect(() => iosSlices(path)).toThrow(/simulator slice/);
  });

  it("stages only the iOS slices and prunes the ones it dropped from Info.plist", async () => {
    const { root, path, slices } = await xcframework();
    const destination = join(root, "Frameworks", "Libbox.xcframework");
    const plist = fakePlist(slices);

    expect(stageLibboxXCFramework({ outputXCFramework: path, destination, plist })).toEqual([
      "ios-arm64",
      "ios-arm64_x86_64-simulator",
    ]);
    expect(
      await readFile(join(destination, "ios-arm64", "Libbox.framework", "Libbox"), "utf8"),
    ).toBe("ios-arm64");
    await expect(
      readFile(join(destination, "macos-arm64_x86_64", "Libbox.framework", "Libbox")),
    ).rejects.toThrow();
    // Back to front, or removing entry 0 would renumber the rest under it.
    expect(plist.removed).toEqual(["AvailableLibraries.3", "AvailableLibraries.0"]);
    // The generated xcframework is a build artifact, not something to keep.
    await expect(readFile(join(path, "Info.plist"))).rejects.toThrow();
  });

  it("says so when the build produced nothing", async () => {
    const root = await temporaryRoot("voyavpn-libbox-ios-missing-");

    expect(() =>
      stageLibboxXCFramework({
        outputXCFramework: join(root, "Libbox.xcframework"),
        destination: join(root, "out"),
        plist: fakePlist([]),
      }),
    ).toThrow(/was not produced/);
  });
});

describe("Libbox Android staging", () => {
  it("copies the archive where Gradle looks for it", async () => {
    const root = await temporaryRoot("voyavpn-libbox-android-");
    const builtAar = join(root, "libbox.aar");
    writeFileSync(builtAar, "aar");
    const destination = join(root, "app", "libs", "libbox.aar");

    expect(stageLibboxAar({ builtAar, destination })).toBe(destination);
    expect(await readFile(destination, "utf8")).toBe("aar");
  });

  it("names the missing archive rather than failing on the copy", async () => {
    const root = await temporaryRoot("voyavpn-libbox-android-missing-");

    expect(() =>
      stageLibboxAar({ builtAar: join(root, "libbox.aar"), destination: join(root, "out.aar") }),
    ).toThrow(/was not produced/);
  });
});
