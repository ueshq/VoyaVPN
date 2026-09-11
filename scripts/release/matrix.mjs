const stableTargets = [
  { os: "macos", arch: "x64", updater: "darwin-x86_64", releaseTarget: "darwin-x86_64" },
  { os: "macos", arch: "arm64", updater: "darwin-aarch64", releaseTarget: "darwin-aarch64" },
  { os: "windows", arch: "x64", updater: "windows-x86_64", releaseTarget: "windows-x86_64" },
  { os: "windows", arch: "arm64", updater: "windows-aarch64", releaseTarget: "windows-aarch64" },
  { os: "linux", arch: "x64", updater: "linux-x86_64", releaseTarget: "linux-x86_64" },
  { os: "linux", arch: "arm64", updater: "linux-aarch64", releaseTarget: "linux-aarch64" },
];

export { stableTargets };
