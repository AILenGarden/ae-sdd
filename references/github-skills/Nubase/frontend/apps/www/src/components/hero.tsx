'use client';

import Link from 'next/link';
import { motion, MotionConfig, type Variants } from 'motion/react';
import { ArrowRight, Star } from 'lucide-react';
import { GoogleOneTap } from '@/components/google-one-tap';
import type { Dict } from '@/lib/i18n';

const GH = 'https://github.com/OtterMind/Nubase';

const EASE = [0.22, 1, 0.36, 1] as const;

const container: Variants = {
  hidden: {},
  show: { transition: { staggerChildren: 0.06, delayChildren: 0.05 } },
};
const item: Variants = {
  hidden: { opacity: 0, y: 12 },
  show: { opacity: 1, y: 0, transition: { duration: 0.5, ease: EASE } },
};

export function Hero({ t }: { t: Dict['hero'] }) {
  return (
    <MotionConfig reducedMotion="user">
      <section className="relative overflow-hidden border-b border-[var(--nb-line)] bg-[var(--nb-bg)] text-[var(--nb-ink)]">
        {/* Google One Tap — auto-hidden unless the backend has Google configured. */}
        <GoogleOneTap />

        {/* clean technical grid — a faint line grid that fades out, theme-aware */}
        <div className="pointer-events-none absolute inset-0 [background-image:linear-gradient(var(--nb-grid)_1px,transparent_1px),linear-gradient(90deg,var(--nb-grid)_1px,transparent_1px)] [background-size:64px_64px] [mask-image:radial-gradient(75%_60%_at_50%_18%,#000_45%,transparent_85%)]" />

        <motion.div
          variants={container}
          initial="hidden"
          animate="show"
          className="container relative z-10 py-28 lg:py-40"
        >
          <div className="mx-auto max-w-3xl text-center">
            <motion.div variants={item} className="flex justify-center">
              <span className="inline-flex items-center gap-2 rounded-full border border-[var(--nb-line)] bg-[var(--nb-surface)] px-3.5 py-1.5 text-[12px] font-medium text-[var(--nb-dim)]">
                <span className="h-1.5 w-1.5 rounded-full bg-[var(--nb-mint)]" />
                {t.badge}
              </span>
            </motion.div>

            <motion.h1
              variants={item}
              className="mt-8 text-balance font-display text-5xl font-semibold leading-[1.05] tracking-tight sm:text-6xl lg:text-[4.5rem]"
            >
              {t.titleA}
              <span className="text-[var(--nb-mint)]">{t.titleHighlight}</span>
              {t.titleEnd}
            </motion.h1>

            <motion.p
              variants={item}
              className="mx-auto mt-6 max-w-lg text-pretty text-lg leading-8 text-[var(--nb-dim)]"
            >
              {t.subtitle}
            </motion.p>

            <motion.div variants={item} className="mt-10 flex flex-wrap items-center justify-center gap-3">
              <Link
                href="/studio/login"
                className="group inline-flex items-center gap-2 rounded-md bg-[var(--nb-mint)] px-5 py-2.5 text-[15px] font-semibold text-[var(--nb-mint-contrast)] transition-opacity hover:opacity-90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--nb-mint)] focus-visible:ring-offset-2 focus-visible:ring-offset-[var(--nb-bg)]"
              >
                {t.ctaPrimary}
                <ArrowRight className="h-4 w-4 transition-transform group-hover:translate-x-0.5" />
              </Link>
              <Link
                href={GH}
                target="_blank"
                rel="noreferrer"
                className="inline-flex items-center gap-2 rounded-md border border-[var(--nb-line)] bg-[var(--nb-surface)] px-5 py-2.5 text-[15px] font-semibold text-[var(--nb-ink)] transition-colors hover:bg-[var(--nb-surface-2)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--nb-mint)] focus-visible:ring-offset-2 focus-visible:ring-offset-[var(--nb-bg)]"
              >
                <Star className="h-4 w-4" /> {t.ctaSecondary}
              </Link>
            </motion.div>
          </div>
        </motion.div>
      </section>
    </MotionConfig>
  );
}
