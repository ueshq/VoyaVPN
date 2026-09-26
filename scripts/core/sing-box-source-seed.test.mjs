import { describe, expect, it } from "vitest";

import { SING_BOX_SOURCE_BUILD_TAGS } from "./sing-box-installer.mjs";
import {
  GO_TOOLCHAIN_ENV,
  goBuildArgs,
  goBuildEnv,
  parseSingBoxVersionOutput,
  requireGoToolchain,
  sourceBuildTags,
  sourceSeedManifest,
} from "./sing-box-source-seed.mjs";

// release/DEFAULT_BUILD_TAGS_OTHERS at v1.13.14.
const upstreamDefaultTags =
  "with_gvisor,with_quic,with_dhcp,with_wireguard,with_utls,with_acme,with_clash_api,with_tailscale,with_ccm,with_ocm,badlinkname,tfogo_checklinkname0\n";

describe("source-built sing-box seed", () => {
  it("builds with upstream's default tags and never the naive outbound", () => {
    expect(sourceBuildTags(upstreamDefaultTags)).toEqual([...SING_BOX_SOURCE_BUILD_TAGS]);
    expect(sourceBuildTags(`${upstreamDefaultTags.trim()},with_naive_outbound`)).not.toContain("with_naive_outbound");
  });

  it("refuses to follow upstream when its default tags drift from the pin", () => {
    expect(() => sourceBuildTags(`${upstreamDefaultTags.trim()},with_v2ray_api`)).toThrow(/SING_BOX_SOURCE_BUILD_TAGS/);
    expect(() => sourceBuildTags("with_gvisor")).toThrow(/Review the upstream change/);
  });

  it("mirrors upstream's make build with the version taken from the pin", () => {
    expect(
      goBuildArgs({
        ldflagsShared: "-X internal/godebug.defaultGODEBUG=multipathtcp=0 -checklinkname=0",
        output: "/tmp/out/sing-box",
        tags: ["with_quic", "with_clash_api"],
        version: "v1.13.14",
      }),
    ).toEqual([
      "build",
      "-trimpath",
      "-ldflags",
      "-X 'github.com/sagernet/sing-box/constant.Version=1.13.14' -X internal/godebug.defaultGODEBUG=multipathtcp=0 -checklinkname=0 -s -w -buildid=",
      "-tags",
      "with_quic,with_clash_api",
      "-o",
      "/tmp/out/sing-box",
      "./cmd/sing-box",
    ]);
    expect(() => goBuildArgs({ output: "x", tags: [], version: "1.13.14; rm -rf /" })).toThrow(/SING_BOX_VERSION/);
  });

  it("uses the installed Go toolchain unless one is named, with cgo on", () => {
    expect(goBuildEnv({ PATH: "/bin" })).toEqual({ CGO_ENABLED: "1", GOTOOLCHAIN: "local", PATH: "/bin" });
    expect(goBuildEnv({ [GO_TOOLCHAIN_ENV]: "go1.25.11", GOTOOLCHAIN: "auto" }).GOTOOLCHAIN).toBe("go1.25.11");
  });

  it("reads the tags and revision a built binary reports", () => {
    const output = `sing-box version 1.13.14

Environment: go1.26.4 darwin/arm64
Tags: with_gvisor,with_quic,badlinkname
Revision: 25a600db24f7680ad9806ce5427bd0ab8afe1114
CGO: enabled
`;
    expect(parseSingBoxVersionOutput(output)).toEqual({
      revision: "25a600db24f7680ad9806ce5427bd0ab8afe1114",
      tags: ["with_gvisor", "with_quic", "badlinkname"],
      version: "1.13.14",
    });
    expect(parseSingBoxVersionOutput("garbage")).toEqual({ revision: null, tags: [], version: null });
  });

  it("records the origin, commit, tags and toolchain in the manifest", () => {
    expect(
      sourceSeedManifest({
        bytes: 10,
        builtAt: "2026-09-26T00:00:00.000Z",
        commit: "25a600db24f7680ad9806ce5427bd0ab8afe1114",
        executableSha256: "a".repeat(64),
        goVersion: "go1.26.4 darwin/arm64",
        kept: ["LICENSE", "sing-box"],
        ldflags: "-s -w",
        pinned: true,
        tags: ["with_quic"],
        target: "darwin-arm64",
        version: "v1.13.14",
      }),
    ).toMatchObject({
      buildScript: "scripts/core/sing-box-source-seed.mjs",
      excludedTags: ["with_naive_outbound"],
      origin: "source",
      sourceRepository: "https://github.com/SagerNet/sing-box.git",
      target: "darwin-arm64",
    });
  });

  it("explains a missing Go toolchain before any work starts", () => {
    expect(requireGoToolchain({ captureCommand: () => ({ status: 0, stdout: "go version go1.26.4 darwin/arm64\n" }) }))
      .toBe("go1.26.4 darwin/arm64");
    expect(() => requireGoToolchain({ captureCommand: () => ({ error: new Error("ENOENT") }) })).toThrow(/brew install go/);
  });
});
