import { useI18n } from "@voya/i18n/use-i18n";
import { TriangleAlert } from "lucide-react-native";
import { Component, type ErrorInfo, type ReactNode, useContext } from "react";
import { View } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";

import { NativeTabSafeAreaContext } from "~/components/use-screen-insets";
import { EmptyState } from "~/components/empty-state";
import { PrimaryButton } from "~/components/primary-button";

type State = { error: Error | null };

/**
 * Keeps a screen that throws while rendering from taking the app with it.
 *
 * React unmounts the whole root when a render error reaches it, and a release
 * build has nothing behind that root: the app closes. With this around a
 * screen the rest of navigation survives, and the screen offers to try again.
 *
 * It has to be a class, so the fallback is a function component that can
 * translate its own words.
 */
export class ScreenErrorBoundary extends Component<{ children: ReactNode }, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("[screen] render error", error, info.componentStack);
  }

  render() {
    if (this.state.error) {
      return <ScreenErrorFallback onRetry={() => this.setState({ error: null })} />;
    }
    return this.props.children;
  }
}

function ScreenErrorFallback({ onRetry }: { onRetry: () => void }) {
  const { t } = useI18n();
  const insets = useSafeAreaInsets();
  const nativeTab = useContext(NativeTabSafeAreaContext);

  return (
    <View
      testID="screen-error-fallback"
      accessibilityRole="alert"
      className="flex-1 justify-center bg-canvas px-page"
      style={{ paddingBottom: nativeTab ? 0 : insets.bottom, paddingTop: nativeTab ? 0 : insets.top }}
    >
      <EmptyState
        icons={[TriangleAlert]}
        title={t("status.screenErrorTitle")}
        description={t("mobile.screenErrorDescription")}
        action={<PrimaryButton label={t("actions.retry")} onPress={onRetry} />}
      />
    </View>
  );
}
