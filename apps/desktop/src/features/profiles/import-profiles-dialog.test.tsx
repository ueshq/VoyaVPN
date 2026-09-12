import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { changeLocale } from "@voya/i18n";
import type { ImportProfilesResult, QrScanResult } from "@/ipc/bindings";

import { ImportProfilesDialog } from "./import-profiles-dialog";
import type { DialogImportMethod } from "./import-methods";
import { QrScanError } from "./qr-errors";

const ipcMocks = vi.hoisted(() => ({
  importProfilesFromText: vi.fn(),
  listSubscriptions: vi.fn(),
  scanClipboardQr: vi.fn(),
}));

const scannerMocks = vi.hoisted(() => ({
  scanQrBlob: vi.fn(),
}));

vi.mock("@/ipc/commands", () => ipcMocks);
vi.mock("./qr-scanner", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./qr-scanner")>();

  return { ...actual, ...scannerMocks };
});

const queryClients = new Set<QueryClient>();

function renderDialog(
  method: DialogImportMethod = "text",
  onImported = vi.fn(),
  onOpenChange = vi.fn(),
) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { gcTime: 0, retry: false } },
  });
  queryClients.add(queryClient);

  const ui = (open: boolean, nextMethod = method) => (
    <QueryClientProvider client={queryClient}>
      <ImportProfilesDialog
        method={nextMethod}
        onImported={onImported}
        onOpenChange={onOpenChange}
        open={open}
      />
    </QueryClientProvider>
  );
  const result = render(ui(true));
  return {
    ...result,
    setOpen: (open: boolean, nextMethod = method) =>
      result.rerender(ui(open, nextMethod)),
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

function clipboardScan(overrides: Partial<QrScanResult> = {}): QrScanResult {
  return {
    failureReason: null,
    message: null,
    source: "clipboard",
    status: "found",
    texts: ["trojan://clipboard.example"],
    ...overrides,
  };
}

describe("ImportProfilesDialog import results", () => {
  it("keeps the dialog open with a localized summary when nothing was imported", async () => {
    const user = userEvent.setup();
    ipcMocks.importProfilesFromText.mockResolvedValue(
      makeImportResult({
        failed: 2,
        messages: ["line 3: unsupported scheme"],
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
    expect(screen.getByText("line 3: unsupported scheme")).toBeInTheDocument();
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

  it("reads a file into the preview, allows retrying the same file and importing only to manual nodes", async () => {
    const user = userEvent.setup();
    ipcMocks.listSubscriptions.mockResolvedValue([
      { id: "sub-target", remarks: "Target subscription" },
    ]);
    renderDialog("file");
    const file = new File(["vless://file"], "nodes.txt", {
      type: "text/plain",
    });
    const read = vi
      .fn()
      .mockRejectedValueOnce(new Error("File unreadable"))
      .mockResolvedValue("vless://file");
    Object.defineProperty(file, "text", { value: read });
    const input = screen.getByLabelText("Import payload file");
    await user.upload(input, file);
    expect(await screen.findByText("File unreadable")).toBeVisible();
    await user.upload(input, file);
    expect(screen.getByLabelText("Import payload")).toHaveValue("vless://file");
    expect(read).toHaveBeenCalledTimes(2);
    expect(ipcMocks.importProfilesFromText).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Import" }));
    expect(ipcMocks.importProfilesFromText).toHaveBeenCalledWith(
      "vless://file",
      null,
    );
  });


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


});

describe("ImportProfilesDialog QR scanning", () => {
  it("fills the editable payload from an image and waits for explicit import", async () => {
    const user = userEvent.setup();
    scannerMocks.scanQrBlob.mockResolvedValue("  vless://image.example  ");
    renderDialog("qrImage");

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

  it("fills the payload from a natively scanned clipboard image without importing it", async () => {
    ipcMocks.scanClipboardQr.mockResolvedValue(
      clipboardScan({ texts: [" trojan://clipboard.example", "vless://b "] }),
    );
    renderDialog("qrClipboard");

    await userEvent.click(
      await screen.findByRole("button", { name: "Clipboard image" }),
    );

    await waitFor(() =>
      expect(screen.getByLabelText("Import payload")).toHaveValue(
        "trojan://clipboard.example\nvless://b",
      ),
    );
    expect(ipcMocks.scanClipboardQr).toHaveBeenCalledOnce();
    expect(scannerMocks.scanQrBlob).not.toHaveBeenCalled();
    expect(ipcMocks.importProfilesFromText).not.toHaveBeenCalled();
  });




  it("shows the localized no-QR result without changing the payload", async () => {
    scannerMocks.scanQrBlob.mockRejectedValue(new QrScanError("notFound"));
    renderDialog("qrImage");

    fireEvent.change(await screen.findByLabelText("Scan image"), {
      target: {
        files: [new File(["plain"], "plain.png", { type: "image/png" })],
      },
    });

    expect(await screen.findByText("No QR code found.")).toBeInTheDocument();
    expect(screen.getByLabelText("Import payload")).toHaveValue("");
    expect(ipcMocks.importProfilesFromText).not.toHaveBeenCalled();
  });

  it.each([
    [
      clipboardScan({ failureReason: "noImage", status: "notFound", texts: [] }),
      "Clipboard does not contain an image.",
    ],
    [clipboardScan({ status: "notFound", texts: [] }), "No QR code found."],
    [
      clipboardScan({
        failureReason: "busy",
        status: "unavailable",
        texts: [],
      }),
      "Could not read an image from the clipboard.",
    ],
    [
      new Error("the clipboard could not be read"),
      "Could not read an image from the clipboard.",
    ],
  ])(
    "translates native clipboard scan result %#",
    async (response, message) => {
      if (response instanceof Error)
        ipcMocks.scanClipboardQr.mockRejectedValue(response);
      else ipcMocks.scanClipboardQr.mockResolvedValue(response);
      renderDialog("qrClipboard");

      await userEvent.click(
        await screen.findByRole("button", { name: "Clipboard image" }),
      );

      expect(await screen.findByText(message)).toBeInTheDocument();
      expect(screen.getByLabelText("Import payload")).toHaveValue("");
      expect(
        screen.queryByText("the clipboard could not be read"),
      ).not.toBeInTheDocument();
    },
  );
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
    messages: [],
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
