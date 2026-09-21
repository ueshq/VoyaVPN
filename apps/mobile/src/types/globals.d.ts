/**
 * Globals the React Native runtime injects that its own types do not declare.
 *
 * `lib` is ES2023 here on purpose — React Native has no DOM, and letting
 * `document`/`window` typecheck would hide real portability bugs until runtime
 * — so anything the runtime provides beyond the language has to be named. The
 * shared client uses `performance.now()` as a monotonic clock; React Native has
 * shipped it in its polyfills for years.
 */
declare const performance: { now: () => number };

/**
 * Hermes has had this since React Native 0.72 and the RN types omit it;
 * `setUpReactDevTools` asserts on it at startup, so it is always there.
 *
 * `structuredClone` deliberately is *not* declared here. It used to be, and it
 * was wrong: RN 0.87 keeps its implementation private to the `Performance` web
 * API and never installs the global. The declaration made shared code compile
 * against a global that does not exist, and the app died on Hermes the first
 * time the settings screen mounted. `no-restricted-globals` now says so too.
 */
declare function queueMicrotask(callback: () => void): void;
