import { createNavigationContainerRef, StackActions, type NavigatorScreenParams } from "@react-navigation/native";
import type { ProxyConnectionItem, RoutingRule } from "@voya/contracts";
import type { ShellTab } from "./tabs";

export type RootRoutes = {
  main: NavigatorScreenParams<Record<ShellTab, undefined>>;
  activity: undefined;
  connectionDetails: { connection: ProxyConnectionItem };
  import: undefined;
  subscriptions: undefined;
  subscription: { id: string };
  general: undefined;
  dns: undefined;
  maintenance: undefined;
  logs: undefined;
  about: undefined;
  ruleDetails: { rule: RoutingRule; target: string };
};
export const navigationRef = createNavigationContainerRef<RootRoutes>();
export function navigateToTab(tab: ShellTab) {
  if (navigationRef.isReady()) navigationRef.dispatch(StackActions.popTo("main", { screen: tab }));
}
export function openPage<Route extends keyof RootRoutes>(...args: undefined extends RootRoutes[Route]
  ? [name: Route, params?: RootRoutes[Route]] : [name: Route, params: RootRoutes[Route]]) {
  if (args[0] === "main") {
    const params = args[1] as RootRoutes["main"];
    if (params && "screen" in params && params.screen) navigateToTab(params.screen);
    return;
  }
  if (navigationRef.isReady()) {
    // navigate's conditional tuple overload cannot retain this correlated generic;
    // dispatch keeps the strongly typed public boundary above.
    navigationRef.dispatch({ type: "NAVIGATE", payload: { name: args[0], params: args[1] } });
  }
}
