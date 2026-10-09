import { environmentValue, repoRootFromScript, runOrExit } from "./lib/common.mjs";
import { commandsUnder, resolveCommand } from "./commands.mjs";

/**
 * The root package.json scripts all land here: `vp run build mac local` runs
 * the `build` script, `node scripts/run.mjs build`, with `mac local` appended.
 * `vp run` takes one task name, so the words after it are resolved against
 * the table in scripts/commands.mjs.
 */
const repoRoot = repoRootFromScript(import.meta.url);
const words = process.argv.slice(2);
const resolved = resolveCommand(words);

if (!resolved) {
  const typed = words.filter((word) => !word.startsWith("-")).join(" ");
  const wantsHelp = words.includes("--help") || words.includes("-h");
  if (!wantsHelp) console.error(`Unknown command: vp run ${typed}`);
  console.error(
    ["Available commands:", ...commandsUnder(words.slice(0, 1)).map((name) => `  vp run ${name}`)].join("\n"),
  );
  process.exit(wantsHelp ? 0 : 1);
}

const { command, args } = resolved;
const programs = {
  node: process.execPath,
  cargo: "cargo",
  // Under `vp run`, vp names its own path; otherwise it is looked up on PATH.
  vp: environmentValue(process.env, "VP_CLI_BIN") || "vp",
};

command.steps.forEach(([program, stepArgs], index) => {
  const last = index === command.steps.length - 1;
  runOrExit(programs[program], last ? [...stepArgs, ...args] : stepArgs, {
    cwd: repoRoot,
    env: { ...process.env, ...command.env },
    log: false,
  });
});
