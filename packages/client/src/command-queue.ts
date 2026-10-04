/**
 * Sends commands one at a time, in the order they were asked for.
 *
 * A stream is switched on and off by two separate commands, and a host that
 * runs commands concurrently makes no promise about which lands last: sent side
 * by side, the "off" of a quick flip of focus or visibility can overtake the
 * "on" before it, and the stream is left running for a screen that has gone —
 * or silent for one that is showing. Chained, the state asked for last is the
 * state that holds. A command that fails does not hold up the ones behind it.
 */
export function createCommandQueue() {
  let last: Promise<unknown> = Promise.resolve();

  return function send<T>(command: () => Promise<T>): Promise<T> {
    const sent = last.then(command);
    last = sent.catch(() => undefined);

    return sent;
  };
}
