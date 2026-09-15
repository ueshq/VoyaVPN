// Tauri plugin APIs the features use directly; tests mock this module.
export { getVersion } from "@tauri-apps/api/app";
export { relaunch } from "@tauri-apps/plugin-process";
export { check, type Update } from "@tauri-apps/plugin-updater";
