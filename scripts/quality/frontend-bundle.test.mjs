import { describe, expect, it } from "vite-plus/test";

import { checkBundleBudgets, checkDistBudgets, checkStartupBudget, startupScripts } from "./frontend-bundle.mjs";

describe("frontend bundle budgets", () => {
  const assets = [
    { name: "index-abc.js", bytes: 50 * 1024 },
    { name: "locales-abc.js", bytes: 40 * 1024 },
    { name: "zh-Hans-abc.js", bytes: 39 * 1024 },
    { name: "zh-Hant-abc.js", bytes: 40 * 1024 },
    { name: "server-table-abc.js", bytes: 130 * 1024 },
    { name: "settings-screen-abc.js", bytes: 64 * 1024 },
    { name: "vendor-data-abc.js", bytes: 79 * 1024 },
    { name: "vendor-react-abc.js", bytes: 186 * 1024 },
    { name: "vendor-radix-abc.js", bytes: 34 * 1024 },
    { name: "vendor-forms-abc.js", bytes: 66 * 1024 },
    { name: "vendor-menus-abc.js", bytes: 122 * 1024 },
  ];

  it("accepts the sizes the budgets were ratcheted around", () => {
    expect(checkBundleBudgets(assets).failures).toEqual([]);
  });

  it("reports every breach rather than throwing on the first", () => {
    const bloated = assets.map((asset) => ({ ...asset, bytes: asset.bytes * 4 }));
    const { failures } = checkBundleBudgets(bloated);

    expect(failures.length).toBe(assets.length + 1);
    expect(failures.at(-1)).toContain("total emitted JavaScript");
  });

  it("fails when a budgeted chunk is not emitted at all", () => {
    const { failures } = checkBundleBudgets(assets.filter((asset) => !asset.name.startsWith("index-")));

    expect(failures).toEqual(["application entry bundle (index-*.js) was not generated"]);
  });

  it("refuses to guess when two chunks answer to one budget", () => {
    // A lazy chunk built from some `index.ts` carries the entry's prefix, and
    // measuring whichever sorts first would pass or fail on the wrong file.
    const twoEntries = [...assets, { name: "index-def.js", bytes: 4 * 1024 }];

    expect(checkBundleBudgets(twoEntries).failures).toEqual([
      "application entry bundle (index-*.js) matches 2 chunks: index-abc.js, index-def.js",
    ]);
  });

  it("catches a total-size regression that no single chunk budget would catch", () => {
    const many = [
      ...assets,
      ...Array.from({ length: 20 }, (_, index) => ({ name: `chunk-${index}.js`, bytes: 40 * 1024 })),
    ];
    const { failures } = checkBundleBudgets(many);

    expect(failures).toHaveLength(1);
    expect(failures[0]).toContain("total emitted JavaScript");
  });
});

describe("frontend startup budget", () => {
  const html = `<!doctype html>
    <script src="/theme-boot.js"></script>
    <script type="module" crossorigin src="/assets/index-abc.js"></script>
    <link rel="modulepreload" crossorigin href="/assets/vendor-react-abc.js">
    <link rel="modulepreload" crossorigin href="/assets/vendor-data-abc.js">
    <link rel="stylesheet" crossorigin href="/assets/index-abc.css">`;
  const sizes = { "assets/index-abc.js": 48, "assets/vendor-react-abc.js": 186, "assets/vendor-data-abc.js": 79 };
  const sizeOf = (script) => sizes[script] * 1024;

  it("counts the entry and its preloads, not the stylesheet or public scripts", () => {
    expect(startupScripts(html)).toEqual([
      "assets/index-abc.js",
      "assets/vendor-react-abc.js",
      "assets/vendor-data-abc.js",
    ]);
    expect(checkStartupBudget(startupScripts(html), sizeOf).failures).toEqual([]);
  });

  it("catches a lazy library pulled onto the startup path", () => {
    const withForms = `${html}<link rel="modulepreload" crossorigin href="/assets/vendor-forms-abc.js">`;
    const bigger = (script) => (script.includes("vendor-forms") ? 300 * 1024 : sizeOf(script));

    expect(checkStartupBudget(startupScripts(withForms), bigger).failures).toEqual([
      expect.stringContaining("startup JavaScript"),
    ]);
  });

  it("fails when index.html loads nothing it could measure", () => {
    expect(checkStartupBudget([], sizeOf).failures).toEqual(["index.html loads no script from assets/"]);
  });
});

describe("frontend dist budgets", () => {
  const files = [
    { name: "index.html", bytes: 2 * 1024 },
    { name: "index-abc.css", bytes: 80 * 1024 },
    { name: "index-abc.js", bytes: 1280 * 1024 },
    { name: "us-abc.svg", bytes: 1650 * 1024 },
  ];

  it("accepts the sizes the budgets were ratcheted around", () => {
    expect(checkDistBudgets(files).failures).toEqual([]);
  });

  it("catches a stylesheet that inlines assets and assets JavaScript budgets never see", () => {
    const flags = [
      ...files,
      { name: "vendor-abc.css", bytes: 421 * 1024 },
      ...Array.from({ length: 142 }, (_, index) => ({ name: `flag-${index}.svg`, bytes: 26 * 1024 })),
    ];

    expect(checkDistBudgets(flags).failures).toEqual([
      expect.stringContaining("total CSS"),
      expect.stringContaining("whole embedded dist"),
    ]);
  });
});
