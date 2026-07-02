'use client';

import { motion, MotionConfig, type Variants } from 'motion/react';
import {
  Brain,
  Clock,
  Database,
  Globe,
  HardDrive,
  KeyRound,
  type LucideIcon,
  Sparkles,
  Zap,
} from 'lucide-react';
import type { Dict } from '@/lib/i18n';

type Mod = { icon: LucideIcon; label: string };

// Module sets per stage (labels stay untranslated — they are product names).
// Stage titles/descriptions come from the dictionary, by index.
const STAGES: { modules: Mod[] }[] = [
  {
    modules: [
      { icon: Database, label: 'Database' },
      { icon: KeyRound, label: 'Auth' },
      { icon: HardDrive, label: 'Storage' },
    ],
  },
  {
    modules: [
      { icon: Zap, label: 'Functions' },
      { icon: Sparkles, label: 'AI Gateway' },
      { icon: Brain, label: 'Memory' },
    ],
  },
  { modules: [{ icon: Globe, label: 'Assets' }] },
  { modules: [{ icon: Clock, label: 'cron' }] },
];

const EASE = [0.22, 1, 0.36, 1] as const;
const VIEWPORT = { once: true, margin: '-80px' } as const;

const list: Variants = {
  hidden: {},
  show: { transition: { staggerChildren: 0.18, delayChildren: 0.1 } },
};
const node: Variants = {
  hidden: { opacity: 0, y: 16 },
  show: { opacity: 1, y: 0, transition: { duration: 0.5, ease: EASE } },
};

export function DeployFlow({ t }: { t: Dict['flow'] }) {
  return (
    <MotionConfig reducedMotion="user">
      <section className="relative overflow-hidden border-b border-[var(--nb-line)] bg-[var(--nb-bg)] py-20 text-[var(--nb-ink)] lg:py-28">
        <div className="pointer-events-none absolute inset-0 [background-image:linear-gradient(var(--nb-grid)_1px,transparent_1px),linear-gradient(90deg,var(--nb-grid)_1px,transparent_1px)] [background-size:64px_64px] [mask-image:radial-gradient(75%_60%_at_50%_15%,#000_45%,transparent_85%)]" />

        <div className="container relative">
          <div className="mx-auto max-w-2xl text-center">
            <p className="text-sm font-semibold uppercase tracking-[0.18em] text-[var(--nb-mint)]">
              {t.eyebrow}
            </p>
            <h2 className="mt-3 font-display text-4xl font-semibold tracking-tight sm:text-5xl">
              {t.title}
            </h2>
            <p className="mt-4 text-[var(--nb-dim)]">{t.subtitle}</p>
          </div>

          {/* the two steps you actually type — the input to the flow below */}
          <motion.div
            initial={{ opacity: 0, y: 16 }}
            whileInView={{ opacity: 1, y: 0 }}
            viewport={VIEWPORT}
            transition={{ duration: 0.5, ease: EASE }}
            className="mx-auto mt-10 max-w-2xl overflow-hidden rounded-lg border border-[var(--nb-line)] bg-[var(--nb-code-bg)] text-left"
          >
            <div className="flex items-center gap-2 border-b border-[var(--nb-line)] px-4 py-2.5 font-mono text-[11px] text-[var(--nb-dim)]">
              <span className="flex gap-1.5">
                <span className="h-2.5 w-2.5 rounded-full bg-[var(--nb-line)]" />
                <span className="h-2.5 w-2.5 rounded-full bg-[var(--nb-line)]" />
                <span className="h-2.5 w-2.5 rounded-full bg-[var(--nb-line)]" />
              </span>
              <span className="ml-2">{t.termHint}</span>
            </div>
            <pre className="overflow-x-auto px-5 py-4 font-mono text-[13px] leading-7 text-[var(--nb-ink)]">
<span className="text-[var(--nb-dim)]">{t.termC1}</span>{'\n'}
<span className="text-[var(--nb-ink)]">npx -y </span><span className="text-[var(--nb-mint)]">nubase_cli@latest</span><span className="text-[var(--nb-ink)]"> install-skills</span>{'\n\n'}
<span className="text-[var(--nb-dim)]">{t.termC2}</span>{'\n'}
<span className="text-[var(--nb-mint)]">“build a notes app and put it online”</span>
            </pre>
          </motion.div>

          <div className="relative mt-20">
            {/* connector line (desktop) — a base rail with a mint progress bar that fills on scroll */}
            <div className="absolute left-[12.5%] right-[12.5%] top-5 hidden h-px bg-[var(--nb-line)] lg:block" />
            <motion.div
              className="absolute left-[12.5%] right-[12.5%] top-5 hidden h-px origin-left bg-[var(--nb-mint)] lg:block"
              initial={{ scaleX: 0 }}
              whileInView={{ scaleX: 1 }}
              viewport={VIEWPORT}
              transition={{ duration: 1.3, ease: EASE, delay: 0.15 }}
            />

            <motion.ol
              variants={list}
              initial="hidden"
              whileInView="show"
              viewport={VIEWPORT}
              className="grid gap-10 lg:grid-cols-4"
            >
              {STAGES.map((s, i) => (
                <motion.li key={i} variants={node} className="text-center">
                  <div className="mx-auto flex h-10 w-10 items-center justify-center rounded-full border border-[var(--nb-mint)]/50 bg-[var(--nb-bg)] font-display text-sm font-semibold text-[var(--nb-mint)] shadow-[0_0_0_4px_var(--nb-bg)]">
                    {i + 1}
                  </div>
                  <h3 className="mt-4 font-display text-lg font-semibold">{t.stages[i]?.title}</h3>
                  <p className="mx-auto mt-1 max-w-[15rem] text-sm leading-6 text-[var(--nb-dim)]">{t.stages[i]?.desc}</p>
                  <ul className="mt-4 flex flex-wrap justify-center gap-1.5">
                    {s.modules.map((m) => {
                      const Icon = m.icon;
                      return (
                        <li key={m.label}>
                          <span className="inline-flex items-center gap-1.5 whitespace-nowrap rounded-md border border-[var(--nb-line)] bg-[var(--nb-surface)] px-2 py-1 text-[12px] font-medium text-[var(--nb-dim)]">
                            <Icon className="h-3 w-3 text-[var(--nb-mint)]" />
                            {m.label}
                          </span>
                        </li>
                      );
                    })}
                  </ul>
                </motion.li>
              ))}
            </motion.ol>
          </div>
        </div>
      </section>
    </MotionConfig>
  );
}
