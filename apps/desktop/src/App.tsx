import { AppShell } from "@/components/app-shell/app-shell";
import { PreferencesBridge } from "@/components/preferences-bridge";
import { EventBridge } from "@/ipc";

export function App() {
  return (
    <>
      <PreferencesBridge />
      <EventBridge />
      <AppShell />
    </>
  );
}
