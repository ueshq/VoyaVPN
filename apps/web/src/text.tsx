import type { Block } from "./content/types";

/** Renders copy text, turning each `backticked` run into <code>. */
export function Inline({ text }: { text: string }) {
  return text.split("`").map((part, index) => (index % 2 === 1 ? <code key={index}>{part}</code> : part));
}

export function Blocks({ blocks }: { blocks: Block[] }) {
  return blocks.map((block, index) =>
    typeof block === "string" ? (
      <p key={index}>
        <Inline text={block} />
      </p>
    ) : (
      <ul key={index} className="list-disc space-y-2 ps-5 marker:text-fg-muted">
        {block.list.map((item) => (
          <li key={item}>
            <Inline text={item} />
          </li>
        ))}
      </ul>
    ),
  );
}
