import { act, renderHook } from "@testing-library/react-native";

import { useBusyAction } from "./use-busy-action";

describe("useBusyAction", () => {
  it("refuses a second action while the first runs, and says it is busy", async () => {
    const { result } = await renderHook(() => useBusyAction());
    let finish!: (value: string) => void;
    const first = jest.fn(() => new Promise<string>((resolve) => (finish = resolve)));
    const second = jest.fn(async () => "second");

    let running!: Promise<string | undefined>;
    await act(async () => {
      running = result.current.run(first);
    });
    expect(result.current.busy).toBe(true);
    // Refused at once — before any render could have disabled the control.
    await expect(result.current.run(second)).resolves.toBeUndefined();
    expect(second).not.toHaveBeenCalled();

    await act(async () => finish("first"));
    await expect(running).resolves.toBe("first");
    expect(result.current.busy).toBe(false);
  });

  it("lets a rejection through and is ready for the next action", async () => {
    const { result } = await renderHook(() => useBusyAction());

    await act(async () => {
      await expect(
        result.current.run(async () => {
          throw new Error("offline");
        }),
      ).rejects.toThrow("offline");
    });

    expect(result.current.busy).toBe(false);
    await act(async () => {
      await expect(result.current.run(async () => 1)).resolves.toBe(1);
    });
  });
});
