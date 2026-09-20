module.exports = {
  presets: ["module:@react-native/babel-preset"],
  plugins: [
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
