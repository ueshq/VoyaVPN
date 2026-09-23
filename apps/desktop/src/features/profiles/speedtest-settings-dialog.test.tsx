import { cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { changeLocale } from "@voya/i18n";

import {
  deferred,
  installSettingsBackend,
  settingsIpc,
} from "@voya/features/settings/settings-backend.test-fixture";
import type { AppSettingsV1 } from "@voya/contracts";
import { renderWithQuery } from "@/test/render";

import { SpeedtestSettingsDialog } from "./speedtest-settings-dialog";

beforeEach(async () => {
  installSettingsBackend();
  await changeLocale("en");
});
afterEach(cleanup);

describe("SpeedtestSettingsDialog", () => {
  it("waits for the settings, then saves a field on its own", async () => {
    const onOpenChange = vi.fn();
    renderWithQuery(<SpeedtestSettingsDialog onOpenChange={onOpenChange} />);

    expect(screen.getByRole("dialog", { name: "Speed test settings" })).toBeInTheDocument();
    expect(screen.getByText("Loading")).toBeInTheDocument();
    const url = await screen.findByLabelText("Latency test URL");

    fireEvent.change(url, { target: { value: "https://example.com/generate_204" } });
    fireEvent.blur(url);
    await waitFor(() =>
      expect(settingsIpc.saveAppSettings).toHaveBeenCalledWith(
        expect.objectContaining({
          speedTest: expect.objectContaining({ latencyUrl: "https://example.com/generate_204" }),
        }),
      ),
    );
    expect(await screen.findByText("Saved. The next speed test uses these settings.")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Done" }));
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });

  it("says it is saving while a save is in flight", async () => {
    const save = deferred<AppSettingsV1>();
    settingsIpc.saveAppSettings.mockReturnValueOnce(save.promise);
    renderWithQuery(<SpeedtestSettingsDialog onOpenChange={vi.fn()} />);

    const timeout = await screen.findByLabelText("Timeout per node (seconds)");
    fireEvent.change(timeout, { target: { value: "7" } });
    fireEvent.blur(timeout);

    expect(await screen.findByText("Saving…")).toBeInTheDocument();
    save.reject(new Error("disk full"));
    await waitFor(() => expect(screen.queryByText("Saving…")).not.toBeInTheDocument());
  });
});
