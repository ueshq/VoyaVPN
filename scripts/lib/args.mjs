/**
 * One CLI argument reader for the release and quality scripts.
 *
 * Every command used to inline the same `next()` closure and switch, which is
 * where alias and flag drift crept in. A spec maps flag names (aliases joined by
 * `|`) to how the value is stored:
 *
 *   "--input"                 -> { key: "input" }                    string
 *   "--out|--output"          -> { key: "output" }                   string with alias
 *   "--allow-empty"           -> { key: "allowEmpty", value: true }  constant, takes no value
 *   "--download-and-hash"     -> { key: "d", value: true, also: { probe: true } }
 *   "--timeout-ms"            -> { key: "timeoutMs", parse: Number }
 *   "--target"                -> { key: "targets", list: true }      comma-split, accumulates
 *
 * `--help` is always accepted and sets `help: true`.
 */
export function parseArgs(argv, spec, defaults = {}) {
  const flags = new Map();
  for (const [names, definition] of Object.entries(spec)) {
    for (const name of names.split("|")) {
      flags.set(name, definition);
    }
  }

  const options = { ...defaults };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--help" || arg === "-h") {
      options.help = true;
      continue;
    }

    const eq = arg.indexOf("=");
    const flagName = eq === -1 ? arg : arg.slice(0, eq);
    const inlineValue = eq === -1 ? null : arg.slice(eq + 1);

    const definition = flags.get(flagName);
    if (!definition) {
      throw new Error(`Unknown argument: ${arg}`);
    }

    if ("value" in definition) {
      if (inlineValue !== null) {
        throw new Error(`${flagName} does not take a value`);
      }
      options[definition.key] = definition.value;
      Object.assign(options, definition.also ?? {});
      continue;
    }

    let raw;
    if (inlineValue !== null) {
      raw = inlineValue;
      if (!raw) {
        throw new Error(`${flagName} requires a value`);
      }
    } else {
      raw = argv[index + 1];
      if (!raw || raw.startsWith("--")) {
        throw new Error(`${flagName} requires a value`);
      }
      index += 1;
    }

    if (definition.list) {
      const values = raw.split(",").map((value) => value.trim()).filter(Boolean);
      options[definition.key] = [...(options[definition.key] ?? []), ...values];
      continue;
    }

    options[definition.key] = definition.parse ? definition.parse(raw) : raw;
  }

  return options;
}
