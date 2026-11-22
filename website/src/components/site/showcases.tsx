import { ClipboardCheck, FolderX, Smartphone } from "lucide-react"
import { motion, useInView, useReducedMotion } from "motion/react"
import { useRef, type ReactNode } from "react"

import CountUp from "@/components/CountUp"
import { OPTIONS } from "@/content/transclip"
import { cn } from "@/lib/utils"

import { InlineCode, Reveal, SectionHeading } from "./bits"

export function Showcases() {
  return (
    <section id="features" className="scroll-mt-16 px-5 py-20 sm:px-8 lg:py-28">
      <SectionHeading
        eyebrow="The details"
        title="Small things, done properly."
      >
        No account, no cloud, no tracking. Everything runs on your Mac.
      </SectionHeading>

      <div className="mx-auto mt-14 grid max-w-6xl gap-5 lg:grid-cols-6">
        <Tile className="bg-apricot lg:col-span-4">
          <PhoneArt />
          <TileText title="It reaches your phone too.">
            Run Transclip over SSH from a phone app such as Termius and the
            transcript lands on your <em>phone's</em> clipboard, not the Mac's.
          </TileText>
        </Tile>

        <Tile className="bg-sage lg:col-span-2" delay={0.08}>
          <SpeedArt />
          <TileText title="Quick when it has to listen.">
            With no captions to borrow, your Mac transcribes the audio itself. A
            15-minute video takes about 30 seconds.
          </TileText>
        </Tile>

        <Tile className="bg-sky lg:col-span-3">
          <TidyArt />
          <TileText title="Nothing kept, nothing sent.">
            Downloads go in a temporary folder that's deleted afterwards, even
            if you stop it partway. The only things kept are the speech model,
            the last error message, a tiny note of when to try captions again,
            and any transcript you choose to save.
          </TileText>
        </Tile>

        <Tile className="bg-card ring-1 ring-line lg:col-span-3" delay={0.08}>
          <h3 className="font-display text-3xl leading-tight font-medium">
            A few extras, if you want them.
          </h3>
          <p className="mt-2 text-ink-soft">
            Add one after <InlineCode>transclip</InlineCode>. You never have to.
          </p>
          <dl className="mt-5 divide-y divide-line">
            {OPTIONS.map((option) => (
              <div
                key={option.flag}
                className="grid gap-x-4 gap-y-0.5 py-2.5 sm:grid-cols-[10.5rem_1fr]"
              >
                <dt className="font-mono text-[13px] font-medium text-coral-deep">
                  {option.flag}
                </dt>
                <dd className="text-sm text-ink-soft">{option.does}</dd>
              </div>
            ))}
          </dl>
        </Tile>
      </div>
    </section>
  )
}

function Tile({
  children,
  className,
  delay,
}: {
  children: ReactNode
  className?: string
  delay?: number
}) {
  return (
    <Reveal
      delay={delay}
      className={cn("rounded-[2rem] p-6 sm:p-9", className)}
    >
      {children}
    </Reveal>
  )
}

function TileText({ title, children }: { title: string; children: ReactNode }) {
  return (
    <div className="mt-8 max-w-xl">
      <h3 className="font-display text-3xl leading-tight font-medium sm:text-4xl">
        {title}
      </h3>
      <p className="mt-3 text-lg text-pretty text-ink/75">{children}</p>
    </div>
  )
}

/** A Mac on one side, a phone on the other, and the transcript crossing over. */
function PhoneArt() {
  const still = useReducedMotion() ?? false
  const ref = useRef<HTMLDivElement>(null)
  const inView = useInView(ref, { amount: 0.5 })
  const moving = inView && !still
  const cycle = { duration: 4.2, repeat: Infinity, ease: "easeInOut" } as const

  return (
    <div
      ref={ref}
      className="flex h-56 items-center gap-2 sm:gap-4"
      aria-hidden
    >
      <div className="w-[46%] max-w-64 shrink-0 overflow-hidden rounded-xl bg-term font-mono text-[10.5px] leading-[1.8] text-term-ink shadow-[0_18px_36px_-20px_rgb(42_28_21/0.6)] sm:text-xs">
        <div className="flex h-6 items-center gap-1.5 bg-term-bar px-2.5">
          <span className="size-2 rounded-full bg-[#ff6259]" />
          <span className="size-2 rounded-full bg-[#f5b942]" />
          <span className="size-2 rounded-full bg-[#3ec55b]" />
          <span className="ml-1 text-[10px] text-term-muted">your Mac</span>
        </div>
        <div className="p-3">
          <p>
            <span className="text-term-muted">~ % </span>transclip
          </p>
          <p>
            <span className="text-term-green">✓</span> Copied to clipboard
          </p>
        </div>
      </div>

      <div className="relative h-8 min-w-8 flex-1">
        <span className="absolute inset-x-0 top-1/2 border-t-2 border-dashed border-ink/25" />
        <span className="absolute top-0 left-1/2 -translate-x-1/2 -translate-y-full text-[11px] font-semibold tracking-widest text-ink/50">
          SSH
        </span>
        <motion.span
          animate={
            moving
              ? { left: ["0%", "0%", "100%", "100%"], opacity: [0, 1, 1, 0] }
              : { left: "50%", opacity: 1 }
          }
          transition={
            moving ? { ...cycle, times: [0, 0.08, 0.5, 0.58] } : { duration: 0 }
          }
          className="absolute top-1/2 grid size-8 -translate-x-1/2 -translate-y-1/2 place-items-center rounded-lg bg-card shadow-md ring-1 ring-line"
        >
          <ClipboardCheck className="size-4 text-coral-deep" />
        </motion.span>
      </div>

      <div className="relative flex h-52 w-28 shrink-0 flex-col rounded-[1.6rem] border-[5px] border-ink bg-card p-2 shadow-[0_18px_36px_-20px_rgb(42_28_21/0.6)] sm:w-32">
        <span className="mx-auto mb-2 h-1.5 w-10 shrink-0 rounded-full bg-ink" />
        <div className="flex-1 rounded-[0.9rem] bg-term p-2 font-mono text-[8.5px] leading-[1.9] text-term-ink">
          <p className="mb-1 flex items-center gap-1 text-term-muted">
            <Smartphone className="size-2.5" />
            your phone
          </p>
          <p>
            <span className="text-term-muted">~ % </span>transclip
          </p>
          <p>
            <span className="text-term-green">✓</span> Copied
          </p>
        </div>
        <motion.span
          animate={
            moving
              ? { opacity: [0, 0, 1, 1, 0], scale: [0.9, 0.9, 1, 1, 0.96] }
              : { opacity: 1, scale: 1 }
          }
          transition={
            moving
              ? { ...cycle, times: [0, 0.5, 0.58, 0.92, 1] }
              : { duration: 0 }
          }
          className="absolute inset-x-3 bottom-4 rounded-lg bg-coral-deep px-2 py-1.5 text-center text-[10px] leading-tight font-semibold text-white"
        >
          On this phone's clipboard
        </motion.span>
      </div>
    </div>
  )
}

/** Two bars drawn to scale: 15 minutes of video, 30 seconds of work. */
function SpeedArt() {
  return (
    <div className="flex h-56 flex-col justify-end gap-5">
      <div>
        <p className="mb-1.5 flex items-baseline justify-between text-sm font-medium text-sage-deep">
          The video
          <span className="font-display text-3xl font-medium text-ink">
            15:00
          </span>
        </p>
        <div className="h-3.5 rounded-full bg-sage-deep/75" />
      </div>
      <div>
        <p className="mb-1.5 flex items-baseline justify-between text-sm font-medium text-sage-deep">
          Transcribing it
          <span className="font-display text-3xl font-medium text-ink">
            0:
            <CountUp to={30} from={0} duration={1.4} />
          </span>
        </p>
        <div className="h-3.5 rounded-full bg-sage-deep/15">
          <motion.div
            initial={{ width: 0 }}
            whileInView={{ width: "3.4%" }}
            viewport={{ once: true }}
            transition={{ duration: 1.2, delay: 0.3, ease: "easeOut" }}
            className="h-full min-w-3.5 rounded-full bg-coral"
          />
        </div>
      </div>
    </div>
  )
}

/** Files drop into a temporary folder, then the whole folder is cleared. */
function TidyArt() {
  const still = useReducedMotion() ?? false
  const ref = useRef<HTMLDivElement>(null)
  const inView = useInView(ref, { amount: 0.5 })
  const moving = inView && !still
  const files = ["audio", "captions", "audio-16k.wav"]

  return (
    <div ref={ref} className="flex h-56 items-end" aria-hidden>
      <div className="w-full rounded-2xl border-2 border-dashed border-sky-deep/35 p-4">
        <p className="mb-3 flex items-center gap-2 text-sm font-semibold text-sky-deep">
          <FolderX className="size-4" />
          Temporary folder
        </p>
        <div className="flex min-h-9 flex-wrap gap-2">
          {files.map((file, i) => (
            <motion.span
              key={file}
              animate={
                moving
                  ? {
                      opacity: [0, 1, 1, 0, 0],
                      y: [-10, 0, 0, 0, 0],
                      scale: [1, 1, 1, 0.8, 0.8],
                    }
                  : { opacity: 0.35 }
              }
              transition={
                moving
                  ? {
                      duration: 5,
                      times: [0, 0.1, 0.6, 0.7, 1],
                      delay: i * 0.25,
                      repeat: Infinity,
                    }
                  : { duration: 0 }
              }
              className={cn(
                "rounded-lg bg-card px-3 py-1.5 font-mono text-xs shadow-sm ring-1 ring-sky-deep/15",
                still && "line-through"
              )}
            >
              {file}
            </motion.span>
          ))}
        </div>
        <p className="mt-3 text-sm text-sky-deep/80">
          Deleted when it finishes.
        </p>
      </div>
    </div>
  )
}
