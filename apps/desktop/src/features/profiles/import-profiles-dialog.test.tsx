import {
  act,
  fireEvent,
  screen,
  waitFor,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { QueryClient } from "@tanstack/react-query";
import { createTestQueryClient, renderWithQuery } from "@/test/render";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { changeLocale } from "@voya/i18n";
import type { ImportProfilesResult } from "@voya/contracts";

import { ImportProfilesDialog } from "./import-profiles-dialog";
import { QrScanError } from "@voya/features/profiles/qr-errors";
import { installFakeCommands, seedListCommands } from "@voya/features/test/backend";

const ipcMocks = installFakeCommands({
  importProfilesFromText: vi.fn(),
  ...seedListCommands(),
});

const scannerMocks = vi.hoisted(() => ({
  scanQrBlob: vi.fn(),
}));

vi.mock("./qr-scanner", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./qr-scanner")>();

  return { ...actual, ...scannerMocks };
});

const queryClients = new Set<QueryClient>();

function renderDialog(onImported = vi.fn(), onOpenChange = vi.fn()) {
  const queryClient = createTestQueryClient({ gcTime: 0 });
  queryClients.add(queryClient);

  const ui = (open: boolean) => (
    <ImportProfilesDialog
      onImported={onImported}
      onOpenChange={onOpenChange}
      open={open}
    />
  );
  const result = renderWithQuery(ui(true), { queryClient });
  return {
    ...result,
    setOpen: (open: boolean) => result.rerender(ui(open)),
    onImported,
    onOpenChange,
  };
}

beforeEach(async () => {
  await changeLocale("en");
  Object.values(ipcMocks).forEach((mock) => mock.mockReset());
  Object.values(scannerMocks).forEach((mock) => mock.mockReset());
  ipcMocks.listSubscriptions.mockResolvedValue([]);
  ipcMocks.importProfilesFromText.mockResolvedValue(
    makeImportResult({
      imported: 1,
      importedProfileIds: ["profile-from-qr"],
      parsed: 1,
    }),
  );
});

afterEach(() => {
  queryClients.forEach((queryClient) => queryClient.clear());
  queryClients.clear();
});

describe("ImportProfilesDialog import results", () => {
  it("keeps the dialog open with a localized summary when nothing was imported", async () => {
    const user = userEvent.setup();
    ipcMocks.importProfilesFromText.mockResolvedValue(
      makeImportResult({
        failed: 2,
        lineIssues: [
          { line: 3, code: { code: "parseFailed", detail: "unsupported scheme" } },
        ],
        parsed: 2,
        skipped: 1,
      }),
    );
    const { onOpenChange } = renderDialog();

    fireEvent.change(await screen.findByLabelText("Import payload"), {
      target: { value: "vless://uuid@example.test:443#US" },
    });
    await user.click(screen.getByRole("button", { name: "Import" }));

    // One import produces exactly one summary: the dialog owns it while it stays
    // open, the profiles banner owns it once it closes.
    expect(
      await screen.findByText(
        "Imported 0 node(s). 1 skipped. 2 failed to parse. Target: Manual import.",
      ),
    ).toBeInTheDocument();
    expect(screen.getByText("Line 3 was skipped: the link is not in a format VoyaVPN can read.")).toBeInTheDocument();
    expect(onOpenChange).not.toHaveBeenCalledWith(false);
  });

  it("closes without a second summary once something was imported", async () => {
    const user = userEvent.setup();
    const { onOpenChange } = renderDialog();

    fireEvent.change(await screen.findByLabelText("Import payload"), {
      target: { value: "vless://uuid@example.test:443#US" },
    });
    await user.click(screen.getByRole("button", { name: "Import" }));

    await waitFor(() => expect(onOpenChange).toHaveBeenCalledWith(false));
    expect(screen.queryByText(/Imported 1 node/)).not.toBeInTheDocument();
  });
});

describe("ImportProfilesDialog sources and lifecycle", () => {

  it("clears the previous preview, target and error on each opening", async () => {
    const user = userEvent.setup();
    ipcMocks.listSubscriptions.mockResolvedValue([
      { id: "sub-target", remarks: "Target subscription" },
    ]);
    ipcMocks.importProfilesFromText.mockRejectedValue(
      new Error("Import failed"),
    );
    const { setOpen } = renderDialog();
    fireEvent.change(screen.getByLabelText("Import payload"), {
      target: { value: "vless://draft" },
    });
    await user.click(screen.getByRole("button", { name: "Import" }));
    expect(await screen.findByText("Import failed")).toBeVisible();
    setOpen(false);
    setOpen(true);
    expect(screen.getByLabelText("Import payload")).toHaveValue("");
    expect(
      screen.queryByRole("combobox", { name: "Target" }),
    ).not.toBeInTheDocument();
    expect(screen.queryByText("Import failed")).not.toBeInTheDocument();
  });

  it("waits for an import result before accepting another submission or dismissal", async () => {
    const user = userEvent.setup();
    let finishImport!: (value: ImportProfilesResult) => void;
    ipcMocks.importProfilesFromText.mockReturnValue(
      new Promise<ImportProfilesResult>((resolve) => {
        finishImport = resolve;
      }),
    );
    const { onOpenChange } = renderDialog();
    fireEvent.change(screen.getByLabelText("Import payload"), {
      target: { value: "vless://draft" },
    });
    const submit = screen.getByRole("button", { name: "Import" });
    await user.click(submit);
    await user.click(submit);
    await user.keyboard("{Escape}");
    expect(onOpenChange).not.toHaveBeenCalled();
    expect(ipcMocks.importProfilesFromText).toHaveBeenCalledOnce();
    expect(screen.getByRole("button", { name: "Close" })).toBeDisabled();
    await act(async () => finishImport(makeImportResult({ failed: 1 })));
    expect(submit).toBeEnabled();
    expect(screen.getByLabelText("Import payload")).toHaveValue(
      "vless://draft",
    );
  });



  it("closes once the text only added subscriptions, leaving their update to the page", async () => {
    ipcMocks.importProfilesFromText.mockResolvedValue(
      makeImportResult({
        addedSubscriptionIds: ["sub-1"],
        lineIssues: [{ line: 1, code: { code: "subscriptionSourceAdded" } }],
      }),
    );
    const { onImported, onOpenChange } = renderDialog();

    fireEvent.change(screen.getByLabelText("Import payload"), {
      target: { value: "https://example.test/subscribe" },
    });
    await userEvent.click(screen.getByRole("button", { name: "Import" }));

    await waitFor(() => expect(onOpenChange).toHaveBeenCalledWith(false));
    expect(onImported).toHaveBeenCalledWith(
      expect.objectContaining({ addedSubscriptionIds: ["sub-1"] }),
    );
  });
});

describe("ImportProfilesDialog QR scanning", () => {
  it("fills the editable payload from an image and waits for explicit import", async () => {
    const user = userEvent.setup();
    scannerMocks.scanQrBlob.mockResolvedValue("  vless://image.example  ");
    renderDialog();

    fireEvent.change(await screen.findByLabelText("Scan image"), {
      target: {
        files: [new File(["qr"], "profile.png", { type: "image/png" })],
      },
    });

    await waitFor(() =>
      expect(screen.getByLabelText("Import payload")).toHaveValue(
        "vless://image.example",
      ),
    );
    expect(ipcMocks.importProfilesFromText).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "Import" }));
    await waitFor(() =>
      expect(ipcMocks.importProfilesFromText).toHaveBeenCalledWith(
        "vless://image.example",
        null,
      ),
    );
  });

  it("shows the localized no-QR result without changing the payload", async () => {
    scannerMocks.scanQrBlob.mockRejectedValue(new QrScanError("notFound"));
    renderDialog();

    fireEvent.change(await screen.findByLabelText("Scan image"), {
      target: {
        files: [new File(["plain"], "plain.png", { type: "image/png" })],
      },
    });

    expect(await screen.findByText("No QR code found.")).toBeInTheDocument();
    expect(screen.getByLabelText("Import payload")).toHaveValue("");
    expect(ipcMocks.importProfilesFromText).not.toHaveBeenCalled();
  });

  it("adds a scanned link below the links already typed", async () => {
    scannerMocks.scanQrBlob.mockResolvedValue("vless://image.example");
    renderDialog();

    fireEvent.change(screen.getByLabelText("Import payload"), {
      target: { value: "trojan://typed.example" },
    });
    fireEvent.change(await screen.findByLabelText("Scan image"), {
      target: { files: [new File(["qr"], "profile.png", { type: "image/png" })] },
    });

    await waitFor(() =>
      expect(screen.getByLabelText("Import payload")).toHaveValue(
        "trojan://typed.example\nvless://image.example",
      ),
    );
  });
});

// Full `ImportProfilesResult` shape; a partial one is what let the dialog paper
// over non-nullable contract fields with `??`.
function makeImportResult(
  overrides: Partial<ImportProfilesResult> = {},
): ImportProfilesResult {
  return {
    deduped: 0,
    discardedNodeOverrides: 0,
    failed: 0,
    filtered: 0,
    imported: 0,
    importedProfileIds: [],
    lineIssues: [],
    addedSubscriptionIds: [],
    parsed: 0,
    removedDuplicates: 0,
    removedExisting: 0,
    skipped: 0,
    subscriptionId: null,
    updated: 0,
    updatedProfileIds: [],
    ...overrides,
  };
}
