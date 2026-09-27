import { COPY } from "../content";
import type { Locale } from "../routes";
import { PRIVACY_EFFECTIVE_DATE, SUPPORT_EMAIL } from "../site";
import { Blocks, Inline } from "../text";

export function PrivacyPage({ locale }: { locale: Locale }) {
  const copy = COPY[locale].privacy;
  const [dateBefore = "", dateAfter = ""] = copy.effective.split("{date}");
  const [emailBefore = "", emailAfter = ""] = copy.contactBody.split("{email}");
  return (
    <article className="mx-auto w-full max-w-3xl px-4 py-14 sm:px-6 md:py-20">
      <h1 className="text-4xl font-semibold tracking-tight">{copy.headline}</h1>
      <p className="mt-3 text-sm text-fg-muted">
        {dateBefore}
        <time dateTime={PRIVACY_EFFECTIVE_DATE}>{PRIVACY_EFFECTIVE_DATE}</time>
        {dateAfter}
      </p>
      <p className="mt-8 text-lg leading-relaxed">
        <Inline text={copy.intro} />
      </p>
      {copy.sections.map((section) => (
        <section key={section.heading} className="mt-12">
          <h2 className="text-xl font-semibold">{section.heading}</h2>
          <div className="mt-3 space-y-4 leading-relaxed text-fg-muted">
            <Blocks blocks={section.body} />
          </div>
        </section>
      ))}
      <section className="mt-12 border-t border-line-muted pt-8">
        <h2 className="text-xl font-semibold">{copy.contactHeading}</h2>
        <p className="mt-3 leading-relaxed text-fg-muted">
          <Inline text={emailBefore} />
          <a className="font-medium text-accent-fg hover:underline" href={`mailto:${SUPPORT_EMAIL}`}>
            {SUPPORT_EMAIL}
          </a>
          <Inline text={emailAfter} />
        </p>
      </section>
    </article>
  );
}
