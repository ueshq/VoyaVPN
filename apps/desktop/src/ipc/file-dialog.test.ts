import { beforeEach, describe, expect, it, vi } from "vitest";
import { save } from "@tauri-apps/plugin-dialog";
import { writeTextFile } from "@tauri-apps/plugin-fs";
import { saveTextFile } from "./file-dialog";

vi.mock("@tauri-apps/plugin-dialog", () => ({ save: vi.fn() }));
vi.mock("@tauri-apps/plugin-fs", () => ({ writeTextFile: vi.fn() }));

const options = {
  defaultPath: "nodes.txt",
  filters: [{ extensions: ["txt"], name: "Text" }],
  text: "节点\nss://example",
};

describe("saveTextFile", () => {
  beforeEach(() => vi.resetAllMocks());

  it("does not write when the user cancels", async () => {
    vi.mocked(save).mockResolvedValue(null);
    expect(await saveTextFile(options)).toBeNull();
    expect(writeTextFile).not.toHaveBeenCalled();
  });

  it("preserves the contents and returns the selected path after writing", async () => {
    vi.mocked(save).mockResolvedValue("/chosen/nodes.txt");
    vi.mocked(writeTextFile).mockResolvedValue();
    expect(await saveTextFile(options)).toBe("/chosen/nodes.txt");
    expect(save).toHaveBeenCalledWith({ defaultPath: options.defaultPath, filters: options.filters });
    expect(writeTextFile).toHaveBeenCalledWith("/chosen/nodes.txt", options.text);
  });

  it("propagates a dialog failure without writing", async () => {
    const failure = new Error("dialog unavailable");
    vi.mocked(save).mockRejectedValue(failure);
    await expect(saveTextFile(options)).rejects.toBe(failure);
    expect(writeTextFile).not.toHaveBeenCalled();
  });

  it("does not report a successful export when the file write fails", async () => {
    const failure = new Error("permission denied");
    vi.mocked(save).mockResolvedValue("/chosen/nodes.txt");
    vi.mocked(writeTextFile).mockRejectedValue(failure);
    await expect(saveTextFile(options)).rejects.toBe(failure);
  });
});
