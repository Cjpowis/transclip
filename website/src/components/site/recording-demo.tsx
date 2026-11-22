import type { Player } from "asciinema-player"
import "asciinema-player/dist/bundle/asciinema-player.css"
import { ClipboardCheck, Pause, Play, RotateCcw } from "lucide-react"
import {
  AnimatePresence,
  motion,
  useInView,
  useReducedMotion,
} from "motion/react"
import { useEffect, useRef, useState, type ReactNode } from "react"

import { DEMO_PACING } from "@/content/demo-pacing"
import { cn } from "@/lib/utils"
import { RECORDINGS, type Recording } from "@/recordings"

/**
 * The hero: real recordings of Transclip, played back in a Terminal
 * window. Nothing in the window is drawn by this site; it is the
 * program's own output, replayed with its own timing.
 */
export function RecordingDemo() {
  const still = useReducedMotion() ?? false
  const [picked, setPicked] = useState(() =>
    Math.max(
      0,
      RECORDINGS.findIndex((r) => r.id === DEMO_PACING.startWith)
    )
  )
  const rec = RECORDINGS[picked]

  return (
    <div className="mx-auto w-full max-w-5xl">
      <div className="mb-5 flex flex-wrap items-center justify-center gap-x-3 gap-y-2">
        <span id="pick-label" className="text-sm font-medium text-ink-soft">
          Watch it with
        </span>
        <div
          role="group"
          aria-labelledby="pick-label"
          className="flex flex-wrap justify-center gap-1 rounded-3xl bg-card p-1 shadow-[0_1px_0_rgb(42_28_21/0.06),0_8px_24px_-12px_rgb(42_28_21/0.25)] ring-1 ring-line sm:rounded-full"
        >
          {RECORDINGS.map((r, i) => (
            <button
              key={r.id}
              type="button"
              aria-pressed={i === picked}
              onClick={() => setPicked(i)}
              className={cn(
                "relative rounded-full px-3.5 py-1.5 text-sm font-medium transition-colors",
                i === picked ? "text-paper" : "text-ink-soft hover:text-ink"
              )}
            >
              {i === picked && (
                <motion.span
                  layoutId="picked-video"
                  className="absolute inset-0 rounded-full bg-ink"
                  transition={{ type: "spring", stiffness: 420, damping: 34 }}
                />
              )}
              <span className="relative">{r.pick}</span>
            </button>
          ))}
        </div>
      </div>

      <Stage key={`${rec.id}-${still}`} rec={rec} still={still} />
    </div>
  )
}

type Status = "loading" | "ready" | "playing" | "paused" | "ended"

function Stage({ rec, still }: { rec: Recording; still: boolean }) {
  const stage = useRef<HTMLDivElement>(null)
  const screen = useRef<HTMLDivElement>(null)
  const player = useRef<Player | null>(null)
  const inView = useInView(stage, { once: true, amount: 0.2 })
  const [status, setStatus] = useState<Status>("loading")
  // With reduced motion the recording opens on the frame where the
  // transcript has just been copied, so the card starts filled in.
  const [copied, setCopied] = useState(still)
  const [copies, setCopies] = useState(0)

  useEffect(() => {
    const el = screen.current
    if (!el) return undefined
    let gone = false

    void import("asciinema-player").then(({ create }) => {
      if (gone) return
      const p = create(rec.cast, el, {
        cols: 80,
        rows: 24,
        preload: true,
        autoPlay: false,
        speed: DEMO_PACING.speed,
        theme: "transclip",
        fit: "width",
        controls: false,
        terminalFontFamily:
          "'JetBrains Mono Variable', ui-monospace, 'SF Mono', Menlo, monospace",
        terminalLineHeight: 1.34,
        poster: `npt:${still ? rec.copiedAt + 1.5 : 0.3}`,
      })
      p.addEventListener("playing", () => setStatus("playing"))
      p.addEventListener("pause", () => setStatus("paused"))
      p.addEventListener("ended", () => setStatus("ended"))
      p.addEventListener("marker", (event) => {
        if (event.label === "copied") setCopied(true)
        if (event.label === "copied-again") setCopies((n) => n + 1)
      })
      player.current = p
      setStatus("ready")
    })

    return () => {
      gone = true
      player.current?.dispose()
      player.current = null
    }
  }, [rec.cast, rec.copiedAt, still])

  // Start by itself once it scrolls into view, unless motion is reduced.
  useEffect(() => {
    if (status === "ready" && inView && !still) void player.current?.play()
  }, [status, inView, still])

  const replay = async () => {
    setCopied(false)
    setCopies(0)
    await player.current?.seek(0)
    await player.current?.play()
  }
  const toggle = () => {
    if (status === "playing") void player.current?.pause()
    else if (status === "ended") void replay()
    else void player.current?.play()
  }

  return (
    <>
      <div
        ref={stage}
        className="relative rounded-[2rem] bg-apricot p-3 sm:rounded-[2.75rem] sm:p-6 lg:p-9"
      >
        <div className="grid items-center gap-4 lg:grid-cols-[minmax(0,1.7fr)_minmax(0,1fr)] lg:gap-0">
          <div className="relative z-0 overflow-hidden rounded-2xl bg-term shadow-[0_2px_0_rgb(255_255_255/0.06)_inset,0_30px_60px_-28px_rgb(42_28_21/0.65),0_12px_24px_-16px_rgb(42_28_21/0.5)] sm:rounded-[1.35rem]">
            <div className="flex h-10 items-center gap-2 bg-term-bar px-4">
              <span className="size-3 rounded-full bg-[#ff6259]" />
              <span className="size-3 rounded-full bg-[#f5b942]" />
              <span className="size-3 rounded-full bg-[#3ec55b]" />
              <span className="flex-1 pr-11 text-center font-mono text-xs text-term-muted">
                Terminal
              </span>
            </div>
            <p className="sr-only">
              A recording of Transclip {rec.version}. The command {rec.command}{" "}
              is typed, a link to “{rec.title}” is pasted, and the transcript,{" "}
              {rec.words} words, is copied to the clipboard and shown on screen.
            </p>
            {/* 80 columns by 24 rows; the ratio holds the space while it loads */}
            <div className="px-2 pt-1 pb-2 sm:px-4 sm:pt-2 sm:pb-4 lg:pr-10">
              <div
                ref={screen}
                aria-hidden
                className="recording aspect-[48/32.4]"
              />
            </div>
          </div>

          <ClipboardCard rec={rec} copied={copied} copies={copies} />
        </div>
      </div>

      <div className="mt-4 flex flex-wrap items-center justify-center gap-x-5 gap-y-2 text-sm text-ink-soft">
        <p>
          A real recording of Transclip {rec.version}
          {DEMO_PACING.speed === 1 ? ", at its real speed" : ""}. The video is
          NASA's.
        </p>
        <span className="flex items-center gap-4">
          <TextButton onPress={toggle} disabled={status === "loading"}>
            {status === "playing" ? (
              <Pause className="size-3.5" aria-hidden />
            ) : (
              <Play className="size-3.5" aria-hidden />
            )}
            {status === "playing" ? "Pause" : "Play"}
          </TextButton>
          <TextButton onPress={replay} disabled={status === "loading"}>
            <RotateCcw className="size-3.5" aria-hidden />
            From the start
          </TextButton>
        </span>
      </div>
    </>
  )
}

function TextButton({
  children,
  onPress,
  disabled,
}: {
  children: ReactNode
  onPress: () => void
  disabled?: boolean
}) {
  return (
    <button
      type="button"
      onClick={onPress}
      disabled={disabled}
      className="inline-flex items-center gap-1.5 font-medium text-ink underline decoration-line decoration-2 underline-offset-4 hover:decoration-coral disabled:opacity-50"
    >
      {children}
    </button>
  )
}

/** Where the words end up: the text Transclip really put on the clipboard. */
function ClipboardCard({
  rec,
  copied,
  copies,
}: {
  rec: Recording
  copied: boolean
  copies: number
}) {
  const [title, link] = rec.clipboard[0].split("\n")
  const body = rec.clipboard.slice(1, 4)

  return (
    <div className="relative z-10 lg:-ml-7">
      <motion.div
        key={copies}
        initial={copies ? { scale: 1.035 } : false}
        animate={{ scale: 1 }}
        transition={{ type: "spring", stiffness: 300, damping: 16 }}
        className="rounded-2xl bg-card p-5 shadow-[0_24px_48px_-24px_rgb(42_28_21/0.45)] ring-1 ring-line lg:rotate-2"
      >
        <div className="mb-3 flex items-center justify-between gap-3">
          <span className="inline-flex items-center gap-2 text-sm font-semibold">
            <ClipboardCheck
              className={cn(
                "size-4 transition-colors duration-500",
                copied ? "text-coral-deep" : "text-ink-soft/50"
              )}
              aria-hidden
            />
            Your clipboard
          </span>
          <AnimatePresence>
            {copied && (
              <motion.span
                initial={{ opacity: 0, scale: 0.6 }}
                animate={{ opacity: 1, scale: 1 }}
                transition={{ type: "spring", stiffness: 500, damping: 22 }}
                className="rounded-full bg-coral-deep px-2.5 py-0.5 text-xs font-semibold whitespace-nowrap text-white"
              >
                {rec.words} words
              </motion.span>
            )}
          </AnimatePresence>
        </div>

        <div className="relative h-48 overflow-hidden text-[13px] leading-relaxed sm:h-52">
          <AnimatePresence mode="wait">
            {copied ? (
              <motion.div
                key="text"
                initial="hidden"
                animate="shown"
                variants={{ shown: { transition: { staggerChildren: 0.09 } } }}
              >
                <Fade className="font-semibold">{title}</Fade>
                <Fade className="truncate text-ink-soft">{link}</Fade>
                {body.map((block) => (
                  <Fade key={block.slice(0, 32)} className="mt-3">
                    {block}
                  </Fade>
                ))}
              </motion.div>
            ) : (
              <motion.div
                key="empty"
                exit={{ opacity: 0 }}
                className="flex h-full flex-col gap-2.5 pt-1"
                aria-hidden
              >
                {[62, 38, 0, 96, 88, 93, 70].map((w, i) =>
                  w ? (
                    <span
                      key={i}
                      className="h-2.5 rounded-full bg-ink/[0.07]"
                      style={{ width: `${w}%` }}
                    />
                  ) : (
                    <span key={i} className="h-1" />
                  )
                )}
              </motion.div>
            )}
          </AnimatePresence>
          <div className="pointer-events-none absolute inset-x-0 bottom-0 h-16 bg-linear-to-t from-card to-transparent" />
        </div>

        <p className="mt-3 border-t border-line pt-3 text-sm text-ink-soft">
          {copied
            ? "Ready to paste into ChatGPT, your notes, or anywhere else."
            : "The transcript will land here."}
        </p>
      </motion.div>
    </div>
  )
}

function Fade({
  children,
  className,
}: {
  children: ReactNode
  className?: string
}) {
  return (
    <motion.p
      variants={{
        hidden: { opacity: 0, y: 8 },
        shown: { opacity: 1, y: 0, transition: { duration: 0.35 } },
      }}
      className={className}
    >
      {children}
    </motion.p>
  )
}
