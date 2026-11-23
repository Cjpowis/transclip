import { ArrowDown } from "lucide-react"
import { motion } from "motion/react"

import { INSTALL_COMMAND, LINKS } from "@/content/transclip"

import { CopyCommand, GitHubMark } from "./bits"
import { RecordingDemo } from "./recording-demo"

export function Header() {
  return (
    <header className="sticky top-0 z-40 border-b border-line/60 bg-paper">
      <div className="mx-auto flex h-16 max-w-6xl items-center justify-between px-5 sm:px-8">
        <a
          href="#top"
          className="inline-flex items-center gap-2.5"
          aria-label="Transclip, back to the top"
        >
          <span className="grid size-8 place-items-center rounded-[0.6rem] bg-coral shadow-[0_6px_14px_-6px_var(--coral)]">
            <svg viewBox="0 0 16 16" className="size-4" aria-hidden>
              <path
                d="M3 4.5h10M3 8h10M3 11.5h6"
                stroke="white"
                strokeWidth="1.8"
                strokeLinecap="round"
              />
            </svg>
          </span>
          <span className="font-display text-[1.55rem] leading-none font-semibold">
            transclip
          </span>
        </a>

        <nav aria-label="Main" className="flex items-center gap-1 sm:gap-2">
          <a
            className="hidden rounded-full px-3 py-2 text-sm font-medium text-ink-soft hover:text-ink sm:block"
            href="#how"
          >
            How it works
          </a>
          <a
            className="hidden rounded-full px-3 py-2 text-sm font-medium text-ink-soft hover:text-ink sm:block"
            href="#install"
          >
            Install
          </a>
          <a
            href={LINKS.repo}
            className="ml-1 inline-flex h-10 items-center gap-2 rounded-full bg-ink px-4 text-sm font-semibold text-paper transition-transform hover:-translate-y-0.5"
          >
            <GitHubMark className="size-4" />
            GitHub
          </a>
        </nav>
      </div>
    </header>
  )
}

export function Hero() {
  return (
    <section
      id="top"
      className="relative overflow-hidden px-5 pt-10 pb-20 sm:px-8 sm:pt-14 lg:pb-28"
    >
      {/* One soft wash of colour behind the headline; nothing else. */}
      <div
        aria-hidden
        className="pointer-events-none absolute inset-x-0 top-0 h-[38rem] bg-[radial-gradient(52rem_26rem_at_50%_-6rem,var(--blush),transparent_70%)] opacity-70"
      />

      <div className="relative mx-auto max-w-4xl text-center">
        <motion.p
          initial={{ opacity: 0, y: 10 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.6 }}
          className="mx-auto mb-6 inline-flex flex-wrap items-center justify-center gap-x-2 gap-y-1 rounded-full bg-card px-4 py-1.5 text-sm font-medium text-ink-soft ring-1 ring-line"
        >
          <span className="size-2 rounded-full bg-coral" aria-hidden />
          Free and open source
          <span aria-hidden className="max-sm:hidden">
            ·
          </span>
          <span className="max-sm:hidden">For Macs with Apple Silicon</span>
        </motion.p>

        <motion.h1
          initial={{ opacity: 0, y: 18 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{
            duration: 0.8,
            delay: 0.08,
            ease: [0.21, 0.6, 0.35, 1],
          }}
          className="font-display text-[2.75rem] leading-[0.98] font-medium text-balance sm:text-7xl lg:text-[5rem]"
        >
          <span className="relative inline-block whitespace-nowrap">
            Every word
            <svg
              viewBox="0 0 300 16"
              preserveAspectRatio="none"
              className="absolute -bottom-[0.14em] left-0 h-[0.22em] w-full overflow-visible"
              aria-hidden
            >
              <motion.path
                d="M3 10 C 45 3, 92 13, 142 7 S 246 4, 297 9"
                fill="none"
                stroke="var(--coral)"
                strokeWidth="5"
                strokeLinecap="round"
                initial={{ pathLength: 0 }}
                animate={{ pathLength: 1 }}
                transition={{ duration: 0.9, delay: 0.75, ease: "easeInOut" }}
              />
            </svg>
          </span>{" "}
          of a YouTube video, on your clipboard.
        </motion.h1>

        <motion.p
          initial={{ opacity: 0, y: 14 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.7, delay: 0.2 }}
          className="mx-auto mt-6 max-w-3xl text-lg text-pretty text-ink-soft sm:text-xl"
        >
          Paste a link. Transclip fetches the captions, or listens to the video
          itself, then copies the whole transcript, ready for ChatGPT, your
          notes, or anywhere else.
        </motion.p>

        <motion.div
          initial={{ opacity: 0, y: 14 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.7, delay: 0.32 }}
          className="mt-8 flex flex-col items-center gap-3.5"
        >
          <CopyCommand
            command={INSTALL_COMMAND}
            label="Install command"
            size="lg"
            className="w-full max-w-[30rem]"
          />
          <a
            href="#install"
            className="inline-flex items-center gap-1.5 text-sm font-medium text-ink-soft hover:text-ink"
          >
            Never used Terminal? It's three steps
            <ArrowDown className="size-3.5" aria-hidden />
          </a>
        </motion.div>
      </div>

      <motion.div
        initial={{ opacity: 0, y: 36 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.9, delay: 0.45, ease: [0.21, 0.6, 0.35, 1] }}
        className="relative mt-10 sm:mt-12"
      >
        <RecordingDemo />
      </motion.div>
    </section>
  )
}
