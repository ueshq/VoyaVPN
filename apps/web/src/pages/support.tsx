import { ChevronDown, Mail } from "lucide-react";
import { COPY } from "../content";
import type { Locale } from "../routes";
import { SUPPORT_EMAIL } from "../site";
import { Blocks, Inline } from "../text";

export function SupportPage({ locale }: { locale: Locale }) {
  const copy = COPY[locale].support;
  return (
    <div className="mx-auto w-full max-w-3xl px-4 py-14 sm:px-6 md:py-20">
      <h1 className="text-4xl font-semibold tracking-tight">{copy.headline}</h1>
      <p className="mt-4 text-lg leading-relaxed text-fg-muted">
        <Inline text={copy.intro} />
      </p>

      <section
        aria-labelledby="contact-heading"
        className="mt-10 rounded-xl border border-line-muted bg-canvas-subtle p-6 md:p-8"
      >
        <h2 id="contact-heading" className="text-xl font-semibold">
          {copy.contactHeadline}
        </h2>
        <p className="mt-2 leading-relaxed text-fg-muted">
          <Inline text={copy.contactBody} />
        </p>
        <a
          href={`mailto:${SUPPORT_EMAIL}`}
          className="mt-5 inline-flex h-11 items-center gap-2 rounded-md bg-accent px-5 font-medium text-white transition-transform hover:brightness-110 active:scale-[0.98]"
        >
          <Mail size={18} strokeWidth={1.75} aria-hidden />
          {SUPPORT_EMAIL}
        </a>
      </section>

      <section aria-labelledby="faq-heading" className="mt-14">
        <h2 id="faq-heading" className="text-2xl font-semibold tracking-tight">
          {copy.faqHeadline}
        </h2>
        <div className="mt-6 border-t border-line-muted">
          {copy.faq.map(({ question, answer }) => (
            <details key={question} className="group border-b border-line-muted">
              <summary className="flex cursor-pointer items-center justify-between gap-4 py-5 font-medium">
                {question}
                <ChevronDown
                  size={18}
                  strokeWidth={1.75}
                  aria-hidden
                  className="shrink-0 text-fg-muted transition-transform group-open:rotate-180 motion-reduce:transition-none"
                />
              </summary>
              <div className="space-y-3 pb-6 leading-relaxed text-fg-muted">
                <Blocks blocks={answer} />
              </div>
            </details>
          ))}
        </div>
      </section>
    </div>
  );
}
