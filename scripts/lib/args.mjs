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
    if (arg === "--help") {
      options.help = true;
      continue;
    }

    const definition = flags.get(arg);
    if (!definition) {
      throw new Error(`Unknown argument: ${arg}`);
    }

    if ("value" in definition) {
      options[definition.key] = definition.value;
      Object.assign(options, definition.also ?? {});
      continue;
    }

    const raw = argv[index + 1];
    if (!raw || raw.startsWith("--")) {
      throw new Error(`${arg} requires a value`);
    }
    index += 1;

    if (definition.list) {
      const values = raw.split(",").map((value) => value.trim()).filter(Boolean);
      options[definition.key] = [...(options[definition.key] ?? []), ...values];
      continue;
    }

    options[definition.key] = definition.parse ? definition.parse(raw) : raw;
  }

  return options;
}
