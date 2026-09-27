import {
  Activity,
  ArrowRight,
  Check,
  EyeOff,
  HardDrive,
  Laptop,
  type LucideIcon,
  MapPin,
  Monitor,
  RadioTower,
  Shield,
  Smartphone,
  Split,
  TabletSmartphone,
  Terminal,
  X,
} from "lucide-react";
import { COPY } from "../content";
import type { FeatureId } from "../content/types";
import { CONTAINER, Screenshot } from "../layout";
import { type Locale, pathFor } from "../routes";
import { DOWNLOADS, type Platform } from "../site";
import { Inline } from "../text";

/** Share-link formats and transports the parser accepts. Names, so never translated. */
const PROTOCOLS = [
  "VLESS",
  "REALITY",
  "VMess",
  "Trojan",
  "Shadowsocks",
  "Hysteria2",
  "TUIC",
  "WireGuard",
  "AnyTLS",
  "SOCKS",
  "HTTP",
  "WebSocket",
  "gRPC",
  "HTTP/2",
  "QUIC",
];

const FEATURE_ICONS: Record<FeatureId, LucideIcon> = {
  rules: Split,
  vpn: Shield,
  selfHost: RadioTower,
  activity: Activity,
  exit: MapPin,
};

const PLATFORMS: { id: Platform; icon: LucideIcon }[] = [
  { id: "macos", icon: Laptop },
  { id: "ios", icon: TabletSmartphone },
  { id: "android", icon: Smartphone },
  { id: "windows", icon: Monitor },
  { id: "linux", icon: Terminal },
];

const ICON = { size: 18, strokeWidth: 1.75, "aria-hidden": true } as const;

export function HomePage({ locale }: { locale: Locale }) {
  const copy = COPY[locale].home;
  return (
    <>
      <section className="bg-[linear-gradient(to_bottom,var(--canvas)_62%,var(--canvas-subtle)_62%)]">
        <div className={`${CONTAINER} pt-14 md:pt-20`}>
          <div className="max-w-3xl">
            <h1 className="text-4xl font-semibold leading-[1.1] tracking-tight text-balance md:text-5xl lg:text-6xl">
              {copy.headline}
            </h1>
            <p className="mt-5 max-w-[52ch] text-lg leading-relaxed text-fg-muted">
              <Inline text={copy.sub} />
            </p>
            <div className="mt-8 flex flex-wrap gap-3">
              <a
                href="#download"
                className="inline-flex h-11 items-center rounded-md bg-accent px-5 font-medium whitespace-nowrap text-white transition-transform hover:brightness-110 active:scale-[0.98]"
              >
                {copy.download}
              </a>
              <a
                href="#features"
                className="inline-flex h-11 items-center gap-2 rounded-md border border-line bg-canvas px-5 font-medium whitespace-nowrap transition-transform hover:bg-canvas-subtle active:scale-[0.98]"
              >
                {copy.features}
              </a>
            </div>
          </div>
          <figure className="mt-12 overflow-hidden rounded-xl border border-line shadow-frame md:mt-16">
            <Screenshot name="home" locale={locale} alt={copy.screenshotAlt} eager />
          </figure>
        </div>
      </section>

      <section className="bg-canvas-subtle" aria-labelledby="privacy-heading">
        <div className={`${CONTAINER} grid gap-10 py-20 md:py-24 lg:grid-cols-12 lg:gap-12`}>
          <div className="lg:col-span-5">
            <h2 id="privacy-heading" className="text-3xl font-semibold tracking-tight text-balance">
              {copy.privacy.headline}
            </h2>
            <p className="mt-4 max-w-[60ch] leading-relaxed text-fg-muted">
              <Inline text={copy.privacy.body} />
            </p>
            <a
              href={pathFor({ locale, page: "privacy" })}
              className="mt-6 inline-flex items-center gap-1.5 font-medium text-accent-fg hover:underline"
            >
              {copy.privacy.link}
              <ArrowRight {...ICON} size={16} />
            </a>
          </div>
          <div className="grid gap-4 sm:grid-cols-2 lg:col-span-7">
            <FactList icon={HardDrive} title={copy.privacy.keptTitle} items={copy.privacy.kept} mark={Check} />
            <FactList icon={EyeOff} title={copy.privacy.neverTitle} items={copy.privacy.never} mark={X} muted />
          </div>
        </div>
      </section>

      <section id="features" className="scroll-mt-4" aria-labelledby="features-heading">
        <div className={`${CONTAINER} py-20 md:py-24`}>
          <h2 id="features-heading" className="text-3xl font-semibold tracking-tight">
            {copy.featuresHeadline}
          </h2>
          <div className="mt-10 grid gap-4 lg:grid-cols-3">
            <article className="flex flex-col overflow-hidden rounded-xl border border-line-muted bg-canvas-subtle lg:col-span-2 lg:row-span-2">
              <div className="p-6 md:p-8">
                <h3 className="text-xl font-semibold">{copy.nodes.title}</h3>
                <p className="mt-2 max-w-[60ch] leading-relaxed text-fg-muted">
                  <Inline text={copy.nodes.body} />
                </p>
              </div>
              <div className="mt-auto ps-6 md:ps-8">
                <div className="overflow-hidden rounded-ss-lg border-s border-t border-line">
                  <Screenshot name="nodes" locale={locale} alt={copy.nodes.alt} />
                </div>
              </div>
            </article>
            {(Object.keys(FEATURE_ICONS) as FeatureId[]).map((id) => (
              <FeatureCell key={id} id={id} locale={locale} />
            ))}
          </div>
        </div>
      </section>

      <section className="border-y border-line-muted bg-canvas-subtle" aria-labelledby="protocols-heading">
        <div className={`${CONTAINER} grid gap-8 py-16 md:py-20 lg:grid-cols-12`}>
          <div className="lg:col-span-5">
            <h2 id="protocols-heading" className="text-2xl font-semibold tracking-tight text-balance">
              {copy.protocolsHeadline}
            </h2>
            <p className="mt-3 max-w-[52ch] leading-relaxed text-fg-muted">
              <Inline text={copy.protocolsBody} />
            </p>
          </div>
          <ul className="flex flex-wrap content-start gap-2 lg:col-span-7">
            {PROTOCOLS.map((name) => (
              <li
                key={name}
                className="rounded-md border border-line bg-canvas px-3 py-1.5 font-mono text-sm"
              >
                {name}
              </li>
            ))}
          </ul>
        </div>
      </section>

      <section id="download" className="scroll-mt-4" aria-labelledby="download-heading">
        <div className={`${CONTAINER} py-20 md:py-24`}>
          <h2 id="download-heading" className="text-3xl font-semibold tracking-tight">
            {copy.downloadHeadline}
          </h2>
          <p className="mt-3 max-w-[60ch] leading-relaxed text-fg-muted">
            <Inline text={copy.downloadBody} />
          </p>
          <ul className="mt-10 grid gap-px overflow-hidden rounded-xl border border-line bg-line md:grid-cols-5">
            {PLATFORMS.map(({ id, icon: Icon }) => {
              const url = DOWNLOADS[id];
              return (
                <li key={id} className="flex items-center gap-3 bg-canvas p-5 md:flex-col md:items-start">
                  <Icon {...ICON} size={22} className="text-fg-muted" />
                  <span className="me-auto font-medium md:me-0">{copy.platforms[id]}</span>
                  {url ? (
                    <a href={url} className="inline-flex items-center gap-1 text-sm font-medium text-accent-fg hover:underline">
                      {copy.getIt}
                      <ArrowRight {...ICON} size={14} />
                    </a>
                  ) : (
                    <span className="text-sm text-fg-muted">{copy.comingSoon}</span>
                  )}
                </li>
              );
            })}
          </ul>
        </div>
      </section>
    </>
  );
}

function FactList({
  icon: Icon,
  mark: Mark,
  title,
  items,
  muted = false,
}: {
  icon: LucideIcon;
  mark: LucideIcon;
  title: string;
  items: string[];
  muted?: boolean;
}) {
  return (
    <div className="rounded-xl border border-line-muted bg-canvas p-6">
      <h3 className="flex items-center gap-2 font-semibold">
        <Icon {...ICON} className="text-fg-muted" />
        {title}
      </h3>
      <ul className="mt-4 space-y-3 text-sm">
        {items.map((item) => (
          <li key={item} className="flex gap-2.5">
            <Mark {...ICON} size={16} className={`mt-0.5 shrink-0 ${muted ? "text-fg-muted" : "text-accent-fg"}`} />
            <span>
              <Inline text={item} />
            </span>
          </li>
        ))}
      </ul>
    </div>
  );
}

function FeatureCell({ id, locale }: { id: FeatureId; locale: Locale }) {
  const Icon = FEATURE_ICONS[id];
  const feature = COPY[locale].home.items[id];
  // The self-hosted node is the one feature most clients lack; the tint marks it.
  const tinted = id === "selfHost";
  return (
    <article
      className={`rounded-xl border p-6 ${tinted ? "border-transparent bg-accent-subtle" : "border-line-muted bg-canvas"}`}
    >
      <span
        className={`inline-flex size-9 items-center justify-center rounded-md border ${tinted ? "border-transparent bg-canvas text-accent-fg" : "border-line-muted bg-canvas-subtle text-fg-muted"}`}
      >
        <Icon {...ICON} />
      </span>
      <h3 className="mt-4 font-semibold">{feature.title}</h3>
      <p className="mt-1.5 text-sm leading-relaxed text-fg-muted">
        <Inline text={feature.body} />
      </p>
    </article>
  );
}
