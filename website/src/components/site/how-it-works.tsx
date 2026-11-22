import { ArrowDown, ArrowRight, Check, Laptop, Link2, Play } from "lucide-react"
import { motion, useInView, useReducedMotion } from "motion/react"
import { useEffect, useRef, useState } from "react"

import { TIMING } from "@/content/demo-pacing"
import { SOURCES, type SourceId } from "@/content/transclip"
import { opening } from "@/recordings"
import { cn } from "@/lib/utils"

import { InlineCode, Reveal, SectionHeading } from "./bits"

const CASES: { id: SourceId; label: string }[] = [
  { id: "captions", label: "has captions" },
  { id: "auto", label: "only has YouTube's auto transcript" },
  { id: "mac", label: "has no text at all" },
]

type CardState = "waiting" | "checking" | "skipped" | "used" | "spare"

const TINT: Record<SourceId, string> = {
  captions: "bg-blush",
  auto: "bg-sky",
  mac: "bg-sage",
}

/**
 * The animated explanation: a link is offered to each source in turn,
 * and the first one that has the words is used.
 */
export function HowItWorks() {
  const still = useReducedMotion() ?? false
  const section = useRef<HTMLDivElement>(null)
  const inView = useInView(section, { amount: 0.35 })

  // `target` is the kind of video chosen; `probe` is the step being tried.
  const [target, setTarget] = useState(0)
  const [probe, setProbe] = useState(0)
  const [auto, setAuto] = useState(true)

  const choose = (index: number, byHand: boolean) => {
    if (byHand) setAuto(false)
    setTarget(index)
    setProbe(0)
  }

  const step = still ? target : Math.min(probe, target)
  const settled = step === target

  useEffect(() => {
    if (still || !inView) return undefined
    if (probe < target) {
      const id = window.setTimeout(
        () => setProbe((p) => p + 1),
        TIMING.explainerStep
      )
      return () => window.clearTimeout(id)
    }
    if (!auto) return undefined
    const id = window.setTimeout(() => {
      setTarget((t) => (t + 1) % CASES.length)
      setProbe(0)
    }, TIMING.explainerHold)
    return () => window.clearTimeout(id)
  }, [still, inView, probe, target, auto])

  const stateOf = (i: number): CardState => {
    if (i < step) return "skipped"
    if (i === step) return settled ? "used" : "checking"
    return settled ? "spare" : "waiting"
  }

  return (
    <section
      id="how"
      className="scroll-mt-16 bg-paper-deep px-5 py-20 sm:px-8 lg:py-28"
    >
      <SectionHeading eyebrow="How it works" title="Where the words come from">
        Transclip takes the first of these that works.
      </SectionHeading>

      <Reveal className="mt-10 flex flex-wrap items-center justify-center gap-x-3 gap-y-2">
        <span id="case-label" className="text-base font-medium text-ink-soft">
          Say the video
        </span>
        <div
          role="group"
          aria-labelledby="case-label"
          className="flex flex-wrap justify-center gap-1 rounded-3xl bg-card p-1 ring-1 ring-line sm:rounded-full"
        >
          {CASES.map((c, i) => (
            <button
              key={c.id}
              type="button"
              aria-pressed={i === target}
              onClick={() => choose(i, true)}
              className={cn(
                "relative rounded-full px-4 py-2 text-sm font-medium transition-colors",
                i === target ? "text-paper" : "text-ink-soft hover:text-ink"
              )}
            >
              {i === target && (
                <motion.span
                  layoutId="picked-case"
                  className="absolute inset-0 rounded-full bg-ink"
                  transition={{ type: "spring", stiffness: 420, damping: 34 }}
                />
              )}
              <span className="relative">{c.label}</span>
            </button>
          ))}
        </div>
      </Reveal>

      <div
        ref={section}
        className="mx-auto mt-14 grid max-w-6xl gap-9 lg:grid-cols-3 lg:gap-7"
      >
        {SOURCES.map((source, i) => {
          const state = stateOf(i)
          const lit = state === "used" || state === "checking"
          return (
            <Reveal key={source.id} delay={i * 0.08} className="relative">
              <motion.article
                animate={{
                  y: state === "used" ? -8 : 0,
                  opacity: lit ? 1 : 0.6,
                }}
                transition={{ type: "spring", stiffness: 260, damping: 26 }}
                className={cn(
                  "relative flex h-full flex-col rounded-[1.75rem] bg-card p-5 ring-1 transition-shadow duration-500 sm:p-6",
                  state === "used"
                    ? "shadow-[0_30px_60px_-30px_rgb(42_28_21/0.45)] ring-ink/25"
                    : "ring-line"
                )}
              >
                {i === step && (
                  <motion.span
                    layoutId="link-token"
                    transition={{ type: "spring", stiffness: 240, damping: 26 }}
                    className="absolute -top-4 left-6 z-10 inline-flex items-center gap-1.5 rounded-full bg-ink px-3 py-1.5 font-mono text-xs text-paper shadow-lg"
                  >
                    <Link2 className="size-3.5" aria-hidden />
                    youtu.be/…
                  </motion.span>
                )}

                <div
                  className={cn(
                    "relative h-44 overflow-hidden rounded-2xl p-4 transition-[filter] duration-500",
                    TINT[source.id],
                    state === "skipped" && "grayscale-[0.85]"
                  )}
                >
                  {source.id === "captions" && (
                    <CaptionsArt playing={state === "used" && !still} />
                  )}
                  {source.id === "auto" && (
                    <AutoArt playing={state === "used" && !still} />
                  )}
                  {source.id === "mac" && (
                    <MacArt playing={state === "used" && !still} />
                  )}
                </div>

                <div className="mt-5 mb-5 flex items-start gap-4">
                  <span className="font-display text-5xl leading-none font-medium text-coral">
                    {i + 1}
                  </span>
                  <div>
                    <h3 className="text-xl leading-snug font-semibold">
                      {source.name}
                    </h3>
                    <p className="mt-1.5 text-ink-soft">{source.blurb}</p>
                  </div>
                </div>

                <p
                  className={cn(
                    "mt-auto flex min-h-11 items-center gap-2 rounded-xl px-3.5 py-2.5 font-mono text-xs transition-colors duration-500",
                    state === "used"
                      ? "bg-term text-term-ink"
                      : "bg-ink/[0.07] text-ink"
                  )}
                >
                  {state === "used" && (
                    <>
                      <Check
                        className="size-3.5 shrink-0 text-term-green"
                        aria-hidden
                      />
                      <span className="rounded-sm bg-term-green px-1.5 font-bold text-term">
                        USED
                      </span>
                    </>
                  )}
                  {state === "skipped" && <span>✗ none</span>}
                  {state === "checking" && <span>checking…</span>}
                  {state === "waiting" && (
                    <span>
                      {i === SOURCES.length - 1 ? "if needed" : "waiting"}
                    </span>
                  )}
                  {state === "spare" && <span>not needed</span>}
                </p>
              </motion.article>

              {i < SOURCES.length - 1 && (
                <span
                  aria-hidden
                  className="absolute -bottom-7 left-1/2 z-10 grid size-7 -translate-x-1/2 place-items-center rounded-full bg-paper text-ink-soft ring-1 ring-line lg:top-1/2 lg:-right-[1.4rem] lg:bottom-auto lg:left-auto lg:translate-x-0 lg:-translate-y-1/2"
                >
                  <ArrowDown className="size-3.5 lg:hidden" />
                  <ArrowRight className="hidden size-3.5 lg:block" />
                </span>
              )}
            </Reveal>
          )
        })}
      </div>

      <Reveal className="mx-auto mt-14 grid max-w-4xl gap-4 text-base text-ink-soft sm:grid-cols-2">
        <p className="rounded-2xl bg-paper p-5 ring-1 ring-line">
          <strong className="font-semibold text-ink">
            Want the most accurate text for names and numbers?
          </strong>{" "}
          Use <InlineCode>transclip -t</InlineCode> to go straight to step 3.
        </p>
        <p className="rounded-2xl bg-paper p-5 ring-1 ring-line">
          <strong className="font-semibold text-ink">
            It knows when to stop asking.
          </strong>{" "}
          If YouTube says captions were asked for too often, Transclip goes
          straight to step 3 for the next 30 minutes rather than asking again.
        </p>
      </Reveal>
    </section>
  )
}

/** Counts up while `playing`, so a little scene can loop. */
function useLoop(playing: boolean, ms: number) {
  const [turn, setTurn] = useState(0)
  useEffect(() => {
    if (!playing) return undefined
    const id = window.setInterval(() => setTurn((t) => t + 1), ms)
    return () => window.clearInterval(id)
  }, [playing, ms])
  return turn
}

function Words({
  text,
  playing,
  turn,
  step = 0.16,
}: {
  text: string
  playing: boolean
  turn: number
  step?: number
}) {
  const words = text.split(" ")
  return (
    <span key={playing ? turn : "still"}>
      {words.map((word, i) => (
        <motion.span
          key={i}
          initial={playing ? { opacity: 0 } : false}
          animate={{ opacity: 1 }}
          transition={{ delay: 0.25 + i * step, duration: 0.15 }}
        >
          {word}{" "}
        </motion.span>
      ))}
    </span>
  )
}

/** A little video with a caption being spoken. */
function CaptionsArt({ playing }: { playing: boolean }) {
  const turn = useLoop(playing, 4200)
  return (
    <div className="relative h-full overflow-hidden rounded-xl bg-term">
      <span className="absolute top-2.5 left-2.5 rounded border border-term-ink/50 px-1 text-[10px] leading-4 font-bold text-term-ink">
        CC
      </span>
      <span className="absolute top-1/2 left-1/2 grid size-10 -translate-x-1/2 -translate-y-[85%] place-items-center rounded-full bg-term-ink/15">
        <Play className="size-4 fill-term-ink text-term-ink" aria-hidden />
      </span>
      <p className="absolute inset-x-3 bottom-5 text-center">
        <span className="rounded bg-black/55 box-decoration-clone px-2 py-1 text-xs leading-6 font-medium text-white">
          <Words text={opening("captions", 11)} playing={playing} turn={turn} />
        </span>
      </p>
      <span className="absolute inset-x-3 bottom-2 h-0.5 overflow-hidden rounded-full bg-term-ink/20">
        <motion.span
          key={playing ? turn : "still"}
          initial={playing ? { width: "8%" } : false}
          animate={{ width: playing ? "62%" : "36%" }}
          transition={{ duration: 4, ease: "linear" }}
          className="block h-full bg-coral"
        />
      </span>
    </div>
  )
}

/** YouTube's own transcript, with the punctuation Transclip looks for. */
function AutoArt({ playing }: { playing: boolean }) {
  const turn = useLoop(playing, 4200)
  const text = opening("auto", 12)
  const parts = text.split(/([,.])/)
  let marks = 0
  return (
    <div className="flex h-full flex-col justify-between rounded-xl bg-card p-3.5 ring-1 ring-sky-deep/15">
      <span className="self-start rounded-full bg-sky px-2 py-0.5 text-[10px] font-semibold tracking-wide text-sky-deep uppercase">
        Auto-generated
      </span>
      <p key={playing ? turn : "still"} className="text-[13px] leading-relaxed">
        {parts.map((part, i) => {
          if (part !== "," && part !== ".") return <span key={i}>{part}</span>
          const order = marks++
          return (
            <motion.span
              key={i}
              initial={
                playing
                  ? { scale: 1, backgroundColor: "rgb(43 95 138 / 0)" }
                  : false
              }
              animate={{
                scale: playing ? [1, 1.9, 1.25] : 1.25,
                backgroundColor: "rgb(43 95 138 / 0.18)",
              }}
              transition={{ delay: 0.4 + order * 0.35, duration: 0.45 }}
              className="mx-px inline-block rounded px-0.5 font-bold text-sky-deep"
            >
              {part}
            </motion.span>
          )
        })}
      </p>
      <span className="inline-flex items-center gap-1 text-xs font-semibold text-sky-deep">
        <Check className="size-3.5" aria-hidden />
        Has punctuation
      </span>
    </div>
  )
}

/** Sound going in, words coming out, all inside the Mac. */
function MacArt({ playing }: { playing: boolean }) {
  const turn = useLoop(playing, 4600)
  const bars = [
    0.5, 0.85, 0.4, 1, 0.65, 0.9, 0.35, 0.75, 1, 0.55, 0.8, 0.45, 0.95, 0.6,
    0.3, 0.7,
  ]
  return (
    <div className="flex h-full flex-col justify-between rounded-xl bg-card p-3.5 ring-1 ring-sage-deep/15">
      <span className="inline-flex items-center gap-1.5 self-start rounded-full bg-sage px-2 py-0.5 text-[10px] font-semibold tracking-wide text-sage-deep uppercase">
        <Laptop className="size-3" aria-hidden />
        On your Mac
      </span>
      <div className="flex h-9 items-center gap-[3px]" aria-hidden>
        {bars.map((h, i) => (
          <span
            key={i}
            style={{
              height: `${h * 100}%`,
              animationDelay: `${(i % 6) * -0.17}s`,
              animationPlayState: playing ? "running" : "paused",
            }}
            className="w-1.5 origin-center rounded-full bg-sage-deep/70 motion-safe:animate-wave"
          />
        ))}
      </div>
      <p className="text-[13px] leading-snug">
        <Words
          text={opening("mac", 11)}
          playing={playing}
          turn={turn}
          step={0.2}
        />
      </p>
    </div>
  )
}
