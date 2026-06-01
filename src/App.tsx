import { AppShell } from "@/components/app-shell/app-shell";
import { EventBridge } from "@/ipc";

export function App() {
  return (
    <>
      <EventBridge />
      <AppShell />
    </>
  );
}
