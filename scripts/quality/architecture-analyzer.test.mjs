import { describe, expect, it } from "vite-plus/test";

import { productionLineCount, splitRustProduction } from "./architecture-analyzer.mjs";

describe("Rust architecture test boundary", () => {
  it("removes a terminal sequence of test modules", () => {
    const source = `use voya_app::Service;

#[cfg(test)]
pub(crate) mod support;

#[cfg(test)]
mod tests {
  #[test]
  fn works() { assert!(true); }
}
`;

    const result = splitRustProduction(source);

    expect(result.layoutError).toBeNull();
    expect(result.production).toBe("use voya_app::Service;\n\n");
  });

  it("does not let later production dependencies or unsafe escape scanning", () => {
    const source = `pub fn before() {}
#[cfg(test)]
mod tests {}
use reqwest::Client;
pub unsafe fn after() {}
`;

    const result = splitRustProduction(source);

    expect(result.layoutError).toContain("terminal test modules");
    expect(result.production).toContain("reqwest::Client");
    expect(result.production).toContain("unsafe fn after");
  });

  it("counts all lines when production follows a test module", () => {
    const hiddenLines = Array.from({ length: 801 }, (_, index) => `pub const LINE_${index}: usize = ${index};`).join("\n");
    const source = `#[cfg(test)]\nmod tests {}\n${hiddenLines}`;
    const result = splitRustProduction(source);

    expect(result.layoutError).not.toBeNull();
    expect(productionLineCount(result.production)).toBeGreaterThan(800);
  });
});
