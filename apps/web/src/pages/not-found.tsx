import { COPY } from "../content";
import { type Locale, pathFor } from "../routes";
import { Inline } from "../text";

export function NotFoundPage({ locale }: { locale: Locale }) {
  const copy = COPY[locale].notFound;
  return (
    <div className="mx-auto w-full max-w-3xl px-4 py-24 sm:px-6 md:py-32">
      <p className="font-mono text-sm text-fg-muted">404</p>
      <h1 className="mt-2 text-4xl font-semibold tracking-tight">{copy.headline}</h1>
      <p className="mt-4 text-lg text-fg-muted">
        <Inline text={copy.body} />
      </p>
      <a
        href={pathFor({ locale, page: "home" })}
        className="mt-8 inline-flex h-11 items-center rounded-md border border-line px-5 font-medium hover:bg-canvas-subtle"
      >
        {copy.back}
      </a>
    </div>
  );
}
