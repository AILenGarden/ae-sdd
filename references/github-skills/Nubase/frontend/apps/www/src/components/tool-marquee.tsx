import { MousePointer2, Terminal, Wind, type LucideIcon } from 'lucide-react';
import type { IconType } from 'react-icons';
import type { Dict } from '@/lib/i18n';
import {
  SiClaude,
  SiGithubcopilot,
  SiGooglegemini,
  SiJetbrains,
  SiNeovim,
  SiOllama,
  SiOpenai,
  SiReplit,
  SiZedindustries,
} from 'react-icons/si';

// Any AI coding agent or editor can drive Nubase over MCP. Real brand logos
// where they exist (react-icons / simple-icons), a neutral icon for the few
// tools without a published mark — all rendered monochrome so the row reads as
// one calm, scrolling strip.
const TOOLS: { Icon: IconType | LucideIcon; label: string }[] = [
  { Icon: SiClaude, label: 'Claude Code' },
  { Icon: SiOpenai, label: 'Codex' },
  { Icon: MousePointer2, label: 'Cursor' },
  { Icon: SiGithubcopilot, label: 'Copilot' },
  { Icon: Wind, label: 'Windsurf' },
  { Icon: Terminal, label: 'Cline' },
  { Icon: SiGooglegemini, label: 'Gemini CLI' },
  { Icon: SiZedindustries, label: 'Zed' },
  { Icon: SiJetbrains, label: 'JetBrains' },
  { Icon: SiNeovim, label: 'Neovim' },
  { Icon: SiReplit, label: 'Replit' },
  { Icon: SiOllama, label: 'Ollama' },
];

export function ToolMarquee({ t }: { t: Dict['marquee'] }) {
  // Rendered twice so the -50% keyframe loops seamlessly.
  const row = [...TOOLS, ...TOOLS];
  return (
    <section className="border-b border-[var(--nb-line)] bg-[var(--nb-bg)] py-9">
      <div className="container">
        <p className="text-center text-xs font-medium uppercase tracking-[0.18em] text-[var(--nb-dim)]">
          {t.label}
        </p>
        <div className="nb-mask-x mt-6 overflow-hidden">
          <ul className="nb-marquee gap-3">
            {row.map((t, i) => {
              const Icon = t.Icon;
              return (
                <li key={i} className="shrink-0">
                  <span className="inline-flex items-center gap-2 whitespace-nowrap rounded-md border border-[var(--nb-line)] bg-[var(--nb-surface)] px-4 py-2 text-sm font-medium text-[var(--nb-dim)] transition-colors hover:bg-[var(--nb-surface-2)] hover:text-[var(--nb-ink)]">
                    <Icon className="h-4 w-4 text-[var(--nb-mint)]" aria-hidden />
                    {t.label}
                  </span>
                </li>
              );
            })}
          </ul>
        </div>
      </div>
    </section>
  );
}
