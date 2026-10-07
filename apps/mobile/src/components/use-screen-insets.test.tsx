import { render } from "@testing-library/react-native";
import { View } from "react-native";
import { SafeAreaInsetsContext } from "react-native-safe-area-context";

import { NativeTabSafeAreaContext, useScreenInsets } from "./use-screen-insets";

function Content() {
  return <View testID="content" style={useScreenInsets()} />;
}

it("does not add system or tab insets a second time inside native tabs", async () => {
  const view = await render(
    <SafeAreaInsetsContext value={{ top: 59, bottom: 34, left: 0, right: 0 }}>
      <NativeTabSafeAreaContext value={true}><Content /></NativeTabSafeAreaContext>
    </SafeAreaInsetsContext>,
  );
  expect(view.getByTestId("content")).toHaveStyle({ paddingTop: 16, paddingBottom: 24 });
});

it("keeps the bottom system inset on pages pushed above the tabs", async () => {
  const view = await render(
    <SafeAreaInsetsContext value={{ top: 59, bottom: 34, left: 0, right: 0 }}>
      <Content />
    </SafeAreaInsetsContext>,
  );
  expect(view.getByTestId("content")).toHaveStyle({ paddingTop: 16, paddingBottom: 58 });
});
