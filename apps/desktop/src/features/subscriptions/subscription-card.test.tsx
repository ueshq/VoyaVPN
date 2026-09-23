import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { i18next } from "@voya/i18n";
import { SubscriptionMetaLine } from "./subscription-card";
import type { SubscriptionMetadata } from "@voya/contracts";
const metadata: SubscriptionMetadata = {
  subscriptionId: "source",
  uploadBytes: 0,
  downloadBytes: 1024,
  totalBytes: 2048,
  expireAt: null,
  lastUpdateAt: null,
  profileTitle: null,
};
describe("SubscriptionMetaLine", () => {
  it("shows reported usage and hides absent server metadata", () => {
    const { rerender, container } = render(
      <SubscriptionMetaLine
        language="en"
        t={i18next.t.bind(i18next)}
        metadata={metadata}
      />,
    );
    expect(screen.getByText(/1.*KB/)).toBeInTheDocument();
    rerender(
      <SubscriptionMetaLine
        language="en"
        t={i18next.t.bind(i18next)}
        metadata={null}
      />,
    );
    expect(container).toBeEmptyDOMElement();
  });
  it("identifies expired sources in words and shows their update time", () => {
    render(
      <SubscriptionMetaLine
        language="en"
        t={i18next.t.bind(i18next)}
        metadata={{
          ...metadata,
          expireAt: 1,
          lastUpdateAt: Math.floor(Date.now() / 1000),
        }}
      />,
    );
    expect(screen.getByText("Expired")).toBeInTheDocument();
    expect(screen.getByText(/Updated/)).toBeInTheDocument();
  });
});
