import { describe, expect, it } from "vitest";

import { redactUrlQuery, urlHost } from "./redact-url-query";

describe("redactUrlQuery", () => {
  it("masks every query value while keeping the keys, path and fragment", () => {
    expect(redactUrlQuery("https://sub.example.com/api/v1/client/subscribe?token=abc123&flag=1#page")).toBe(
      "https://sub.example.com/api/v1/client/subscribe?token=…&flag=…#page",
    );
  });

  it("returns a URL without a query untouched", () => {
    expect(redactUrlQuery("https://example.com/plain")).toBe("https://example.com/plain");
  });

  it("keeps a key that carries no value", () => {
    expect(redactUrlQuery("https://example.com/a?bare&x=1")).toBe("https://example.com/a?bare=…&x=…");
  });
});

describe("urlHost", () => {
  it("reads the host without its userinfo, port, path or query", () => {
    expect(urlHost("https://sub.example.test/path?token=abc#frag")).toBe("sub.example.test");
    expect(urlHost("http://user:pass@example.test:8080/x")).toBe("example.test");
    expect(urlHost("https://[2001:db8::1]:443/sub")).toBe("[2001:db8::1]");
  });

  it("answers with nothing, rather than throwing, for text that is not a URL", () => {
    expect(urlHost("example.test/path#frag")).toBe("");
    expect(urlHost("")).toBe("");
  });
});
