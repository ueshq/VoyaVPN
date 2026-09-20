module.exports = {
  preset: "@react-native/jest-preset",
  setupFilesAfterEnv: ["<rootDir>/src/test/setup.ts"],
  moduleNameMapper: {
    "^~/(.*)$": "<rootDir>/src/$1",
  },
  // The React Native preset transforms `.js`/`.ts`/`.tsx` only. `@gluestack-ui`
  // ships its ESM build as `.jsx`, so without this entry those files reach the
  // runtime untranspiled and fail with "Cannot use import statement outside a
  // module".
  transform: {
    "^.+\\.(js|jsx|ts|tsx)$": "babel-jest",
  },
  // React Native's packages ship untranspiled ESM and have to go through Babel.
  // The usual `node_modules/(?!(react-native|…)/)` form does not work under
  // pnpm: the real path is `node_modules/.pnpm/<name>@<version>_<hash>/…`, so
  // the segment right after the first `node_modules/` is always `.pnpm`. This
  // asks the question that actually matters instead — does the path belong to
  // one of these packages at all?
  //
  // The `@voya/*` packages need no entry: they resolve through a workspace
  // symlink to `packages/*/src`, a path with no `node_modules` in it, so they
  // are never ignored.
  transformIgnorePatterns: [
    "node_modules/(?!.*(react-native|@react-navigation|test-renderer|uniwind|@gluestack-ui|@legendapp))",
  ],
};
