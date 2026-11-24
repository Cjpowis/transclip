import { Search } from "lucide-react"
import { motion, useReducedMotion } from "motion/react"
import type { ReactNode } from "react"

import {
  GOOD_TO_KNOW,
  HOMEBREW_COMMAND,
  INSTALL_COMMAND,
  LINKS,
} from "@/content/transclip"

import {
  CopyCommand,
  GitHubMark,
  InlineCode,
  Kbd,
  Reveal,
  SectionHeading,
} from "./bits"

export function Install() {
  return (
    <section
      id="install"
      className="scroll-mt-16 bg-paper-deep px-5 py-20 sm:px-8 lg:py-28"
    >
      <SectionHeading
        eyebrow="Install"
        title="Never used Terminal? It's three steps."
      >
        Terminal is an app already on your Mac. You type a line, press Return,
        and it does the rest.
      </SectionHeading>

      <ol className="mx-auto mt-14 grid max-w-6xl grid-cols-[minmax(0,1fr)] gap-5 lg:grid-cols-3">
        <Step number={1} title="Open Terminal">
          <SpotlightArt />
          <p>
            Press <Kbd>⌘</Kbd> and <Kbd>Space</Kbd>, type{" "}
            <strong className="font-semibold text-ink">Terminal</strong>, press{" "}
            <Kbd>Return</Kbd>.
          </p>
        </Step>

        <Step number={2} title="Paste this and press Return" delay={0.08}>
          <CopyCommand
            command={INSTALL_COMMAND}
            label="Install command"
            stacked
          />
          <p>
            This also installs{" "}
            <strong className="font-semibold text-ink">yt-dlp</strong> and{" "}
            <strong className="font-semibold text-ink">ffmpeg</strong>, which
            Transclip uses.
          </p>
        </Step>

        <Step number={3} title="Type transclip" delay={0.16}>
          <div className="rounded-xl bg-term px-4 py-3 font-mono text-sm text-term-ink">
            <span className="text-term-muted">~ % </span>transclip
            <span className="ml-px inline-block h-[1.1em] w-[0.58em] translate-y-[0.2em] bg-term-ink motion-safe:animate-caret" />
          </div>
          <p>
            Press Return, then paste a YouTube link and press Return. The
            transcript is copied to your clipboard straight away and shown on
            screen. Press <InlineCode>c</InlineCode> to copy it again,{" "}
            <InlineCode>s</InlineCode> to save it as a text file,{" "}
            <InlineCode>q</InlineCode> to quit.
          </p>
        </Step>
      </ol>

      <Reveal className="mx-auto mt-5 max-w-6xl rounded-[2rem] bg-card p-6 ring-1 ring-line sm:p-9">
        <div className="grid grid-cols-[minmax(0,1fr)] gap-6 lg:grid-cols-[minmax(0,1fr)_minmax(0,1.25fr)] lg:items-center lg:gap-12">
          <div>
            <h3 className="font-display text-3xl leading-tight font-medium">
              Step 2 says it can't find “brew”?
            </h3>
            <p className="mt-3 text-ink-soft">
              Then you need Homebrew first. Paste this line (it's the one from{" "}
              <a
                className="font-medium text-coral-deep underline underline-offset-4"
                href={LINKS.homebrew}
              >
                brew.sh
              </a>
              ) and press Return, then do step 2 again.
            </p>
          </div>
          <div>
            <CopyCommand
              command={HOMEBREW_COMMAND}
              label="Homebrew install command"
            />
            <ul className="mt-4 space-y-2 text-sm text-ink-soft">
              <li className="flex gap-2">
                <Dot />
                It asks for your Mac password. Nothing shows as you type, which
                is normal.
              </li>
              <li className="flex gap-2">
                <Dot />
                When it finishes it prints “Next steps” with a couple of lines
                to paste. Paste them, or Terminal won't find brew.
              </li>
            </ul>
          </div>
        </div>
      </Reveal>
    </section>
  )
}

function Dot() {
  return (
    <span
      className="mt-2 size-1.5 shrink-0 rounded-full bg-coral"
      aria-hidden
    />
  )
}

function Step({
  number,
  title,
  children,
  delay,
}: {
  number: number
  title: string
  children: ReactNode
  delay?: number
}) {
  return (
    <li className="list-none">
      <Reveal
        delay={delay}
        className="flex h-full flex-col gap-5 rounded-[2rem] bg-card p-6 ring-1 ring-line sm:p-8"
      >
        <div className="flex items-center gap-4">
          <span className="grid size-11 shrink-0 place-items-center rounded-full bg-coral font-display text-2xl font-medium text-white">
            {number}
          </span>
          <h3 className="text-xl leading-snug font-semibold">{title}</h3>
        </div>
        <div className="flex flex-1 flex-col gap-4 text-ink-soft [&>p]:leading-relaxed">
          {children}
        </div>
      </Reveal>
    </li>
  )
}

/** ⌘ and Space being pressed, and "Terminal" appearing in Spotlight. */
function SpotlightArt() {
  const still = useReducedMotion() ?? false
  const press = (delay: number) =>
    still
      ? {}
      : {
          animate: {
            y: [0, 3, 0],
            boxShadow: [
              "0 3px 0 rgb(42 28 21 / 0.2)",
              "0 0px 0 rgb(42 28 21 / 0.2)",
              "0 3px 0 rgb(42 28 21 / 0.2)",
            ],
          },
          transition: {
            duration: 0.5,
            delay,
            repeat: Infinity,
            repeatDelay: 3.3,
          },
        }
  const key =
    "grid h-11 place-items-center rounded-xl bg-paper text-base font-medium ring-1 ring-line shadow-[0_3px_0_rgb(42_28_21/0.2)]"

  return (
    <div className="flex flex-col gap-3" aria-hidden>
      <div className="flex gap-2">
        <motion.span {...press(0)} className={`${key} w-12`}>
          ⌘
        </motion.span>
        <motion.span {...press(0.12)} className={`${key} flex-1 text-sm`}>
          Space
        </motion.span>
      </div>
      <div className="flex h-11 items-center gap-2.5 rounded-xl bg-paper px-3.5 ring-1 ring-line">
        <Search className="size-4 text-ink-soft" />
        <span className="text-ink">Terminal</span>
      </div>
    </div>
  )
}

export function GoodToKnow() {
  return (
    <section className="px-5 py-24 sm:px-8">
      <div className="mx-auto grid max-w-6xl gap-10 lg:grid-cols-[1fr_1.4fr] lg:gap-16">
        <Reveal>
          <h2 className="font-display text-4xl leading-[1.05] font-medium sm:text-5xl">
            Good to know.
          </h2>
          <p className="mt-4 text-lg text-ink-soft">
            Transclip is new, and honest about what it can't do yet.
          </p>
        </Reveal>
        <Reveal delay={0.08}>
          <ul className="divide-y divide-line border-y border-line">
            {GOOD_TO_KNOW.map((item) => (
              <li key={item} className="flex gap-3 py-4 text-lg text-pretty">
                <span
                  className="mt-2.5 size-2 shrink-0 rounded-full bg-coral"
                  aria-hidden
                />
                {item}
              </li>
            ))}
          </ul>
        </Reveal>
      </div>
    </section>
  )
}

export function Footer() {
  const link =
    "underline decoration-line decoration-2 underline-offset-4 hover:decoration-coral"
  return (
    <footer className="bg-ink px-5 py-16 text-paper sm:px-8">
      <div className="mx-auto max-w-6xl">
        <div className="flex flex-col items-start justify-between gap-8 border-b border-paper/15 pb-10 lg:flex-row lg:items-center">
          <div>
            <p className="font-display text-4xl leading-tight font-medium sm:text-5xl">
              Go on, try it on a video.
            </p>
            <p className="mt-3 text-lg text-paper/70">
              Free and open source, under the MIT licence.
            </p>
          </div>
          <a
            href={LINKS.repo}
            className="inline-flex h-12 items-center gap-2.5 rounded-full bg-paper px-6 font-semibold text-ink transition-transform hover:-translate-y-0.5"
          >
            <GitHubMark className="size-5" />
            Transclip on GitHub
          </a>
        </div>

        <div className="mt-10 grid gap-8 text-sm text-paper/70 lg:grid-cols-[auto_1fr] lg:gap-16 [&_a]:text-paper [&_a:focus-visible]:outline-paper">
          <ul className="flex flex-wrap gap-x-6 gap-y-2 font-medium lg:flex-col">
            <li>
              <a className={link} href={LINKS.discussions}>
                Ask a question
              </a>
            </li>
            <li>
              <a className={link} href={LINKS.issues}>
                Report a problem
              </a>
            </li>
            <li>
              <a className={link} href={LINKS.changelog}>
                What's changed
              </a>
            </li>
            <li>
              <a className={link} href={LINKS.licence}>
                MIT licence
              </a>
            </li>
          </ul>
          <div className="max-w-3xl space-y-3 leading-relaxed">
            <p>
              For a problem, run{" "}
              <code className="font-mono text-paper">transclip --report</code>{" "}
              and paste what it copies into the issue. It leaves out links,
              video IDs and your folder names.
            </p>
            <p>
              Made by{" "}
              <a className={link} href={LINKS.author}>
                Cjpowis
              </a>
              . Built on{" "}
              <a className={link} href={LINKS.ytdlp}>
                yt-dlp
              </a>
              ,{" "}
              <a className={link} href={LINKS.ffmpeg}>
                FFmpeg
              </a>
              ,{" "}
              <a className={link} href={LINKS.parakeetRs}>
                parakeet-rs
              </a>{" "}
              and NVIDIA's{" "}
              <a className={link} href={LINKS.model}>
                parakeet-unified-en-0.6b
              </a>
              , which is licensed by NVIDIA Corporation under the{" "}
              <a className={link} href={LINKS.modelLicence}>
                NVIDIA Open Model License
              </a>
              . Not affiliated with YouTube.
            </p>
          </div>
        </div>
      </div>
    </footer>
  )
}
