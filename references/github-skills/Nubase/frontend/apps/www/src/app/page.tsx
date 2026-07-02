import Link from 'next/link';
import {
  ArrowRight,
  Boxes,
  Brain,
  Clock,
  Database,
  Github,
  Globe,
  HardDrive,
  KeyRound,
  Layers,
  type LucideIcon,
  LifeBuoy,
  MessagesSquare,
  Rocket,
  ShieldCheck,
  Sparkles,
  Star,
  Zap,
} from 'lucide-react';
import { DeployFlow } from '@/components/deploy-flow';
import { Hero } from '@/components/hero';
import { ToolMarquee } from '@/components/tool-marquee';
import { getLang } from '@/lib/get-lang';
import { getDict } from '@/lib/i18n';

const GH = 'https://github.com/OtterMind/Nubase';

// Product/module names stay untranslated; bodies/tags/stats come from the dict by index.
type TagKey = 'publishFrontend' | 'deployLogic' | 'firstClass';
const PRIMITIVES: { icon: LucideIcon; title: string; tag?: TagKey }[] = [
  { icon: Database, title: 'Database' },
  { icon: KeyRound, title: 'Auth' },
  { icon: HardDrive, title: 'Storage' },
  { icon: Globe, title: 'Assets', tag: 'publishFrontend' },
  { icon: Zap, title: 'Functions', tag: 'deployLogic' },
  { icon: Sparkles, title: 'AI Gateway' },
  { icon: Brain, title: 'Memory', tag: 'firstClass' },
  { icon: Clock, title: 'cron' },
];

const STAT_VALUES = ['8', '1', '1', '0'];

const INTEGRATIONS = [
  'OpenAI', 'Anthropic', 'PostgreSQL', 'pgvector', 'Cloudflare R2',
  'Cloudflare Workers', 'AWS S3', 'MinIO', 'Docker', 'MCP',
];

const LEARN: { icon: LucideIcon; href: string }[] = [
  { icon: Rocket, href: '/docs/getting-started' },
  { icon: MessagesSquare, href: GH },
  { icon: LifeBuoy, href: '/docs/concepts' },
];

const TRUST_ICONS: LucideIcon[] = [ShieldCheck, Layers, Boxes, Github];

export default function Home() {
  const t = getDict(getLang());

  return (
    <main className="overflow-hidden">
      <Hero t={t.hero} />

      <ToolMarquee t={t.marquee} />

      <DeployFlow t={t.flow} />

      {/* ======================================================== TOOLCHAIN */}
      <section className="bg-[var(--nb-bg-2)] py-16 lg:py-24">
        <div className="container">
          <div className="mx-auto max-w-2xl text-center">
            <p className="text-sm font-semibold uppercase tracking-[0.18em] text-[var(--nb-green)]">
              {t.toolchain.eyebrow}
            </p>
            <h2 className="mt-3 font-display text-4xl font-semibold text-[var(--nb-ink)] sm:text-5xl">
              {t.toolchain.title}
            </h2>
            <p className="mt-4 text-[var(--nb-dim)]">{t.toolchain.subtitle}</p>
          </div>

          <div className="mt-14 grid gap-5 sm:grid-cols-2 lg:grid-cols-4">
            {PRIMITIVES.map((p, i) => {
              const Icon = p.icon;
              return (
                <div key={p.title} className="nb-card flex flex-col p-7">
                  <div className="flex items-center justify-between">
                    <div className="flex h-12 w-12 items-center justify-center rounded-xl bg-[var(--nb-mint-soft)] text-[var(--nb-green-deep)]">
                      <Icon className="h-6 w-6" />
                    </div>
                    {p.tag && (
                      <span className="rounded-full bg-[var(--nb-mint-soft)] px-2.5 py-1 text-[11px] font-semibold uppercase tracking-wide text-[var(--nb-green-deep)]">
                        {t.toolchain.tags[p.tag]}
                      </span>
                    )}
                  </div>
                  <h3 className="mt-5 font-display text-xl font-semibold text-[var(--nb-ink)]">{p.title}</h3>
                  <p className="mt-2 text-sm leading-6 text-[var(--nb-dim)]">{t.toolchain.bodies[i]}</p>
                </div>
              );
            })}
          </div>

          {/* stats strip */}
          <div className="mt-14 grid overflow-hidden rounded-2xl border border-[var(--nb-line)] bg-[var(--nb-surface)] sm:grid-cols-2 lg:grid-cols-4">
            {STAT_VALUES.map((n, i) => (
              <div key={i} className={`p-7 ${i > 0 ? 'border-t border-[var(--nb-line)] sm:border-t-0 sm:border-l lg:border-t-0' : ''}`}>
                <div className="font-display text-5xl font-semibold text-[var(--nb-green-deep)]">{n}</div>
                <div className="mt-2 text-sm text-[var(--nb-dim)]">{t.toolchain.stats[i]}</div>
              </div>
            ))}
          </div>
        </div>
      </section>

      {/* ===================================================== INTEGRATIONS */}
      <section className="bg-[var(--nb-bg)] py-16 lg:py-20">
        <div className="container">
          <div className="mx-auto max-w-2xl text-center">
            <p className="text-sm font-semibold uppercase tracking-[0.18em] text-[var(--nb-green)]">
              {t.integrations.eyebrow}
            </p>
            <h2 className="mt-3 font-display text-4xl font-semibold text-[var(--nb-ink)] sm:text-5xl">
              {t.integrations.title}
            </h2>
            <p className="mt-4 text-[var(--nb-dim)]">{t.integrations.subtitle}</p>
          </div>

          <div className="mt-12 grid grid-cols-2 gap-4 sm:grid-cols-3 lg:grid-cols-4">
            {INTEGRATIONS.map((name) => (
              <div
                key={name}
                className="flex items-center justify-center rounded-xl border border-[var(--nb-line)] bg-[var(--nb-surface)] px-4 py-5 text-center font-display text-base font-semibold text-[var(--nb-ink)] transition-colors hover:border-[var(--nb-green)]/40"
              >
                {name}
              </div>
            ))}
          </div>
        </div>
      </section>

      {/* ============================================================ CTA */}
      <section className="bg-[var(--nb-bg-2)] py-16 lg:py-20">
        <div className="container">
          <div className="relative overflow-hidden rounded-2xl border border-[var(--nb-line)] bg-[var(--nb-surface)] px-8 py-12 lg:px-16 lg:py-14">
            <div className="flex flex-col items-center gap-8 text-center lg:flex-row lg:items-center lg:justify-between lg:text-left">
              <div className="flex items-center gap-5">
                <div className="hidden h-16 w-16 shrink-0 items-center justify-center rounded-xl bg-[var(--nb-mint-soft)] text-[var(--nb-mint)] sm:flex">
                  <Rocket className="h-8 w-8" />
                </div>
                <div>
                  <h2 className="font-display text-3xl font-semibold text-[var(--nb-ink)] sm:text-4xl">
                    {t.cta.title}
                  </h2>
                  <p className="mt-2 max-w-xl text-[var(--nb-dim)]">{t.cta.body}</p>
                </div>
              </div>
              <Link
                href="/docs/getting-started"
                className="inline-flex shrink-0 items-center gap-2 rounded-md bg-[var(--nb-mint)] px-5 py-2.5 text-[15px] font-semibold text-[var(--nb-mint-contrast)] transition-opacity hover:opacity-90"
              >
                {t.cta.button} <ArrowRight className="h-4 w-4" />
              </Link>
            </div>
          </div>
        </div>
      </section>

      {/* ========================================================= LEARN/GROW */}
      <section className="bg-[var(--nb-bg)] py-16 lg:py-24">
        <div className="container">
          <div className="mx-auto max-w-2xl text-center">
            <p className="text-sm font-semibold uppercase tracking-[0.18em] text-[var(--nb-green)]">
              {t.learn.eyebrow}
            </p>
            <h2 className="mt-3 font-display text-4xl font-semibold text-[var(--nb-ink)] sm:text-5xl">
              {t.learn.title}
            </h2>
          </div>

          <div className="mt-12 grid gap-5 md:grid-cols-3">
            {LEARN.map((c, i) => {
              const Icon = c.icon;
              const card = t.learn.cards[i];
              return (
                <div key={c.href} className="nb-card flex flex-col p-7">
                  <div className="flex h-12 w-12 items-center justify-center rounded-xl bg-[var(--nb-mint-soft)] text-[var(--nb-green-deep)]">
                    <Icon className="h-6 w-6" />
                  </div>
                  <h3 className="mt-5 font-display text-xl font-semibold text-[var(--nb-ink)]">{card?.title}</h3>
                  <p className="mt-2 flex-1 text-sm leading-6 text-[var(--nb-dim)]">{card?.body}</p>
                  <Link
                    href={c.href}
                    className="mt-5 inline-flex items-center gap-1.5 text-sm font-semibold text-[var(--nb-green)] transition-colors hover:text-[var(--nb-green-deep)]"
                  >
                    {card?.cta} <ArrowRight className="h-4 w-4" />
                  </Link>
                </div>
              );
            })}
          </div>

          {/* trust row */}
          <div className="mt-12 flex flex-wrap items-center justify-center gap-x-8 gap-y-3 text-sm font-medium text-[var(--nb-dim)]">
            {TRUST_ICONS.map((Ic, i) => {
              const I = Ic;
              return (
                <span key={i} className="inline-flex items-center gap-2">
                  <I className="h-4 w-4 text-[var(--nb-green)]" /> {t.learn.trust[i]}
                </span>
              );
            })}
          </div>
        </div>
      </section>

      {/* ========================================================= FINAL CTA */}
      <section className="bg-[var(--nb-bg-2)] py-16 lg:py-24">
        <div className="container">
          <div className="relative overflow-hidden rounded-2xl border border-[var(--nb-line)] bg-[var(--nb-surface)] px-8 py-16 text-center lg:px-16 lg:py-20">
            <div className="pointer-events-none absolute inset-0 [background-image:linear-gradient(var(--nb-grid)_1px,transparent_1px),linear-gradient(90deg,var(--nb-grid)_1px,transparent_1px)] [background-size:48px_48px] [mask-image:radial-gradient(70%_70%_at_50%_30%,#000_40%,transparent_85%)]" />
            <div className="relative">
              <Github className="mx-auto h-8 w-8 text-[var(--nb-mint)]" />
              <h2 className="mx-auto mt-5 max-w-3xl text-balance font-display text-4xl font-semibold text-[var(--nb-ink)] sm:text-5xl">
                {t.finalCta.title}
              </h2>
              <p className="mx-auto mt-4 max-w-xl text-[var(--nb-dim)]">{t.finalCta.body}</p>
              <div className="mt-9 flex flex-wrap items-center justify-center gap-3">
                <Link
                  href="/studio/login"
                  className="inline-flex items-center gap-2 rounded-md bg-[var(--nb-mint)] px-5 py-2.5 text-[15px] font-semibold text-[var(--nb-mint-contrast)] transition-opacity hover:opacity-90"
                >
                  {t.finalCta.primary} <ArrowRight className="h-4 w-4" />
                </Link>
                <Link
                  href={GH}
                  target="_blank"
                  rel="noreferrer"
                  className="inline-flex items-center gap-2 rounded-md border border-[var(--nb-line)] bg-[var(--nb-surface)] px-5 py-2.5 text-[15px] font-semibold text-[var(--nb-ink)] transition-colors hover:bg-[var(--nb-surface-2)]"
                >
                  <Star className="h-4 w-4" /> {t.finalCta.secondary}
                </Link>
              </div>
            </div>
          </div>
        </div>
      </section>
    </main>
  );
}
