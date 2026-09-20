import { describe, expect, it } from "vitest";

import type { AppError } from "@voya/contracts";

import { appErrorOfKind, IpcCommandError, unwrapCommandResult } from "./errors";

function appError(kind: AppError["kind"]): AppError {
  return { kind, subsystem: "app", message: "diagnostic text" };
}

describe("unwrapCommandResult", () => {
  it("returns the data of a successful command", () => {
    expect(unwrapCommandResult({ status: "ok", data: 42 })).toBe(42);
  });

  it("throws the backend error, keeping it typed on the thrown value", () => {
    const error = appError({ type: "elevationRequired" });

    try {
      unwrapCommandResult({ status: "error", error });
      expect.unreachable("expected a throw");
    } catch (thrown) {
      expect(thrown).toBeInstanceOf(IpcCommandError);
      expect((thrown as IpcCommandError).appError).toBe(error);
      // The diagnostic reaches `Error.message` so a log or toast can show it.
      expect((thrown as IpcCommandError).message).toBe("diagnostic text");
    }
  });
});

describe("appErrorOfKind", () => {
  it("narrows a matching error to its kind", () => {
    const error = new IpcCommandError(
      appError({ type: "notFound", entity: "profile", id: "node-1" }),
    );

    const matched = appErrorOfKind(error, "notFound");

    expect(matched?.kind.entity).toBe("profile");
  });

  it("returns null for a different kind", () => {
    const error = new IpcCommandError(appError({ type: "network" }));

    expect(appErrorOfKind(error, "elevationRequired")).toBeNull();
  });

  it("returns null for anything that is not a command failure", () => {
    // Substring-matching `message` is what this guard exists to replace, so a
    // plain Error carrying the same words must not match.
    expect(appErrorOfKind(new Error("elevationRequired"), "elevationRequired")).toBeNull();
    expect(appErrorOfKind(null, "network")).toBeNull();
  });
});
