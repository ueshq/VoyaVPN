module.exports = {
  presets: ["module:@react-native/babel-preset"],
  env: {
    // Metro turns `import()` into its own async require at bundle time, so the
    // React Native preset only teaches Babel to *parse* it. Jest runs the
    // output in a CommonJS VM with no such callback, where a real dynamic
    // import throws `ERR_VM_DYNAMIC_IMPORT_CALLBACK_MISSING_FLAG` — which is
    // how `@voya/i18n` loads the Chinese bundles.
    test: { plugins: ["@babel/plugin-transform-dynamic-import"] },
  },
  plugins: [
    // zod ships `export * as x from …`, which Metro's preset does not handle;
    // the schemas in `@voya/features` are compiled from source, so the app's
    // Babel config is where it has to be handled.
    "@babel/plugin-transform-export-namespace-from",
    [
      "module-resolver",
      {
        // `@/*` is desktop-private (AGENTS.md); the mobile app uses `~` for the
        // same job. The TypeScript side of this alias lives in tsconfig.json.
        root: ["./src"],
        alias: { "~": "./src" },
        extensions: [".ts", ".tsx", ".js", ".jsx", ".json"],
      },
    ],
  ],
};
