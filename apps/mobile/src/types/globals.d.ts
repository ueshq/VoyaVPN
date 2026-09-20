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
