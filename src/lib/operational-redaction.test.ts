import { describe, expect, it } from "vitest";

import { redactOperationalError, redactOperationalMessage } from "@/lib/operational-redaction";

describe("operational redaction", () => {
  it("redacts URLs, share links, and sensitive assignments", () => {
    const redacted = redactOperationalMessage(
      "failed at https://user:pass@example.test/webdav proxyUrl=http://127.0.0.1:1080 UserName=alice password: hunter2 vless://secret@example.test",
    );

    expect(redacted).toBe(
      "failed at [redacted URL] proxyUrl=[redacted] UserName=[redacted] password: [redacted] [redacted]",
    );
    expect(redacted).not.toContain("example.test");
    expect(redacted).not.toContain("127.0.0.1");
    expect(redacted).not.toContain("alice");
    expect(redacted).not.toContain("hunter2");
    expect(redacted).not.toContain("vless://");
  });

  it("supports localized placeholders for rendered messages", () => {
    expect(
      redactOperationalError(new Error("download failed at https://cdn.example.test/app"), {
        redactedUrl: "[URL]",
        redactedValue: "[VALUE]",
      }),
    ).toBe("download failed at [URL]");
  });
});
