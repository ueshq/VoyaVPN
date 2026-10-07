import { act, render } from "@testing-library/react-native";
import { appVisibilityAdapter, setAppVisibility } from "@voya/client/platform";
import { useToastStore } from "@voya/client/toast-store";
import { Alert } from "react-native";

import { NoticeHost } from "./notice-host";

beforeEach(() => {
  useToastStore.setState({ toasts: [] });
  jest.spyOn(Alert, "alert").mockImplementation(() => {});
});

afterEach(() => {
  jest.restoreAllMocks();
});

describe("NoticeHost", () => {
  it("shows an error or a warning as one alert, then forgets it", async () => {
    await render(<NoticeHost />);

    await act(async () => {
      useToastStore.getState().pushToast({
        description: "exit code 2",
        severity: "error",
        title: "The core stopped",
      });
    });

    expect(Alert.alert).toHaveBeenCalledTimes(1);
    expect(Alert.alert).toHaveBeenCalledWith("The core stopped", "exit code 2");
    expect(useToastStore.getState().toasts).toEqual([]);
  });

  it("drops a milder notice without interrupting anyone", async () => {
    await render(<NoticeHost />);

    await act(async () => {
      useToastStore.getState().pushToast({ severity: "info", title: "Copied" });
    });

    expect(Alert.alert).not.toHaveBeenCalled();
    expect(useToastStore.getState().toasts).toEqual([]);
  });

  it("keeps a notice that arrives in the background until the app is on screen again", async () => {
    let visible = false;
    const listeners = new Set<() => void>();
    const previous = appVisibilityAdapter();
    setAppVisibility({
      isVisible: () => visible,
      subscribe: (onChange) => {
        listeners.add(onChange);
        return () => {
          listeners.delete(onChange);
        };
      },
    });
    await render(<NoticeHost />);

    await act(async () => {
      useToastStore.getState().pushToast({ severity: "error", title: "The core stopped" });
    });

    expect(Alert.alert).not.toHaveBeenCalled();
    expect(useToastStore.getState().toasts).toHaveLength(1);

    await act(async () => {
      visible = true;
      for (const listener of listeners) listener();
    });

    expect(Alert.alert).toHaveBeenCalledWith("The core stopped", undefined);
    expect(useToastStore.getState().toasts).toEqual([]);
    setAppVisibility(previous);
  });

  it("works through what queued up before it mounted, in order", async () => {
    const { pushToast } = useToastStore.getState();
    pushToast({ severity: "warning", title: "First" });
    pushToast({ severity: "error", title: "Second" });

    await render(<NoticeHost />);

    expect((Alert.alert as jest.Mock).mock.calls.map(([title]) => title)).toEqual(["First", "Second"]);
    expect(useToastStore.getState().toasts).toEqual([]);
  });
});
