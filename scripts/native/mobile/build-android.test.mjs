import { describe, expect, it } from "vite-plus/test";

import {
  apkProblems,
  parseApkSigner,
  parseKeytoolSha256,
  resolveAndroidVersionCode,
  resolveApkPath,
  signingProblems,
  signingVariables,
} from "./build-android.mjs";

const signingEnv = Object.fromEntries(signingVariables.map((name) => [name, "value"]));
const keySha256 = "AB:CD:EF:01";
const entries = ["arm64-v8a", "x86_64"].flatMap((abi) => [`lib/${abi}/libvoya_mobile_ffi.so`, `lib/${abi}/libbox.so`]);
const goodApk = {
  badging:
    "package: name='app.voyavpn.mobile' versionCode='412' versionName='0.1.0' platformBuildVersionName='16'\n" +
    "native-code: 'arm64-v8a' 'x86_64'\n",
  signer: { dn: "CN=VoyaVPN", sha256: "abcdef01" },
  keySha256,
  entries,
  version: "0.1.0",
  versionCode: "412",
};

describe("Android production APK lane", () => {
  it("requires all four signing variables and an existing keystore", () => {
    expect(signingProblems(signingEnv, () => true)).toEqual([]);
    expect(signingProblems({ ...signingEnv, VOYAVPN_ANDROID_KEY_ALIAS: " " }, () => true)).toEqual([
      "VOYAVPN_ANDROID_KEY_ALIAS is not set.",
    ]);
    expect(signingProblems({}, () => true)[0]).toMatch(/^VOYAVPN_ANDROID_KEYSTORE, .* are not set\.$/u);
    expect(signingProblems(signingEnv, () => false)).toEqual([
      "VOYAVPN_ANDROID_KEYSTORE points at value, which does not exist.",
    ]);
  });

  it("takes the version code from the environment or the commit count", () => {
    const git = () => ({ status: 0, stdout: "412\n" });
    expect(resolveAndroidVersionCode({ env: {}, repoRoot: "/repo", captureCommand: git })).toBe("412");
    expect(
      resolveAndroidVersionCode({ env: { VOYAVPN_ANDROID_VERSION_CODE: "7" }, repoRoot: "/repo", captureCommand: git }),
    ).toBe("7");
    expect(() =>
      resolveAndroidVersionCode({ env: { VOYAVPN_ANDROID_VERSION_CODE: "1.2" }, repoRoot: "/repo" }),
    ).toThrow(/integer/u);
  });

  it("reads the signer and the key's fingerprint", () => {
    expect(
      parseApkSigner(
        "Signer #1 certificate DN: CN=VoyaVPN, O=Voya\nSigner #1 certificate SHA-256 digest: abcdef01\nSigner #1 certificate SHA-1 digest: 99\n",
      ),
    ).toEqual({ dn: "CN=VoyaVPN, O=Voya", sha256: "abcdef01" });
    expect(
      parseApkSigner(
        "V2 Signer: certificate DN: CN=VoyaVPN\nV2 Signer: certificate SHA-256 digest: abcdef01\nV2 Signer: certificate SHA-1 digest: 99\n",
      ),
    ).toEqual({ dn: "CN=VoyaVPN", sha256: "abcdef01" });
    expect(parseKeytoolSha256("Certificate fingerprints:\n\t SHA1: 99:88\n\t SHA256: AB:CD:EF:01\n")).toBe(keySha256);
  });

  it("accepts an APK that matches the release", () => {
    expect(apkProblems(goodApk)).toEqual([]);
  });

  it("rejects the debug keystore, a debuggable build, wrong versions and a missing library", () => {
    expect(
      apkProblems({
        ...goodApk,
        badging: `package: name='app.voyavpn.mobile' versionCode='1' versionName='0.0.9'\napplication-debuggable\n`,
        signer: { dn: "CN=Android Debug, O=Android, C=US", sha256: "1234" },
        entries: entries.filter((entry) => entry !== "lib/x86_64/libbox.so"),
      }),
    ).toEqual([
      "versionCode is 1, expected 412.",
      "versionName is 0.0.9, expected 0.1.0.",
      "The APK is debuggable.",
      "The APK is signed by CN=Android Debug, O=Android, C=US, not by the release key.",
      "lib/x86_64/libbox.so is missing.",
    ]);
  });

  it("names the APK after the version and version code", () => {
    expect(resolveApkPath({ outputDir: "/out", version: "0.1.0", versionCode: "412" })).toBe(
      "/out/VoyaVPN_0.1.0_412.apk",
    );
  });
});
