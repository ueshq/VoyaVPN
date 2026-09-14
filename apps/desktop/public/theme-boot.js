// Applies the saved theme before the app renders, so a dark window never
// flashes light at launch. It reads the store preferences-store.ts persists
// ("voyavpn.preferences"); PreferencesBridge takes over once React mounts.
// A separate file rather than an inline script, because the app's CSP only
// allows scripts from its own origin.
(function applySavedTheme() {
  const root = document.documentElement;
  let mode = "system";
  try {
    const saved = JSON.parse(window.localStorage.getItem("voyavpn.preferences") || "null");
    if (saved && saved.state && typeof saved.state.themeMode === "string") {
      mode = saved.state.themeMode;
    }
  } catch {
    // Unreadable storage falls back to the system theme.
  }
  const dark =
    mode === "dark" ||
    (mode !== "light" &&
      typeof window.matchMedia === "function" &&
      window.matchMedia("(prefers-color-scheme: dark)").matches);
  root.classList.toggle("dark", dark);
  root.style.colorScheme = dark ? "dark" : "light";
})();
