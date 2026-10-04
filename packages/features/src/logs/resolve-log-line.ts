import { logLineText } from "@voya/client/messages";
import type { StoredLogLine } from "@voya/client/runtime-event-store";
import type { TranslationFunction } from "@voya/i18n/core";
import { formatTimeOfDay } from "@voya/utils/formatting";

export type ResolvedLogLine = StoredLogLine & {
  /** The message alone, in the interface language. */
  text: string;
  /** The local time of day it was logged at. */
  time: string;
  /** `time [level] message`: the line as a log file reads, and as it is copied. */
  stamped: string;
  /** `stamped` in lower case, made once so a search only compares. */
  searchText: string;
};

/**
 * Resolved lines, keyed by the stored line. The store keeps each line's object
 * from frame to frame and hands over a new array of up to five hundred of them
 * on every batch, several times a second; a line is translated and stamped
 * once per language, and an entry leaves with its line.
 */
const resolvedLines = new WeakMap<
  StoredLogLine,
  { resolved: ResolvedLogLine; t: TranslationFunction }
>();

/**
 * One stored line as both log views show, search and export it. The text is
 * what was logged: a log is read to find out what happened, and a line with
 * its addresses blanked no longer says.
 */
export function resolveLogLine(t: TranslationFunction, line: StoredLogLine): ResolvedLogLine {
  const cached = resolvedLines.get(line);
  if (cached?.t === t) return cached.resolved;
  const text = logLineText(t, line.body);
  const time = formatTimeOfDay(line.loggedAt);
  const stamped = `${time} [${line.level}] ${text}`;
  const resolved = { ...line, searchText: stamped.toLowerCase(), stamped, text, time };
  resolvedLines.set(line, { resolved, t });
  return resolved;
}
