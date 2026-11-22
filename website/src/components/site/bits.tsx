import { Check, Copy } from "lucide-react"
import { AnimatePresence, motion } from "motion/react"
import { useEffect, useRef, useState, type ReactNode } from "react"

import { cn } from "@/lib/utils"

/** Fades a block up into place the first time it scrolls into view. */
export function Reveal({
  children,
  className,
  delay = 0,
}: {
  children: ReactNode
  className?: string
  delay?: number
}) {
  return (
    <motion.div
      initial={{ opacity: 0, y: 28 }}
      whileInView={{ opacity: 1, y: 0 }}
      viewport={{ once: true, margin: "0px 0px -12% 0px" }}
      transition={{ duration: 0.7, delay, ease: [0.21, 0.6, 0.35, 1] }}
      className={className}
    >
      {children}
    </motion.div>
  )
}

export function Eyebrow({ children }: { children: ReactNode }) {
  return (
    <p className="mb-4 text-sm font-semibold tracking-wide text-coral-deep uppercase">
      {children}
    </p>
  )
}

export function SectionHeading({
  eyebrow,
  title,
  children,
  className,
}: {
  eyebrow?: string
  title: ReactNode
  children?: ReactNode
  className?: string
}) {
  return (
    <Reveal className={cn("mx-auto max-w-3xl text-center", className)}>
      {eyebrow && <Eyebrow>{eyebrow}</Eyebrow>}
      <h2 className="font-display text-4xl leading-[1.05] font-medium text-balance sm:text-5xl lg:text-6xl">
        {title}
      </h2>
      {children && (
        <p className="mx-auto mt-5 max-w-2xl text-lg text-pretty text-ink-soft sm:text-xl">
          {children}
        </p>
      )}
    </Reveal>
  )
}

/** A command in a box, with a button that copies it. */
export function CopyCommand({
  command,
  label,
  size = "md",
  stacked = false,
  className,
}: {
  command: string
  /** What the command is, for screen readers. */
  label: string
  size?: "md" | "lg"
  /** Command on one line, button underneath: for narrow cards. */
  stacked?: boolean
  className?: string
}) {
  const [state, setState] = useState<"idle" | "copied" | "failed">("idle")
  const code = useRef<HTMLElement>(null)
  const timer = useRef<number | undefined>(undefined)

  useEffect(() => () => window.clearTimeout(timer.current), [])

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(command)
      setState("copied")
    } catch {
      // No clipboard access: select the text so ⌘C works.
      if (code.current) {
        const range = document.createRange()
        range.selectNodeContents(code.current)
        const selection = window.getSelection()
        selection?.removeAllRanges()
        selection?.addRange(range)
      }
      setState("failed")
    }
    window.clearTimeout(timer.current)
    timer.current = window.setTimeout(() => setState("idle"), 2200)
  }

  return (
    <div
      className={cn(
        "flex max-w-full items-center gap-2 rounded-2xl bg-card py-1.5 pr-1.5 pl-4 text-left shadow-[0_1px_0_rgb(42_28_21/0.05),0_14px_30px_-18px_rgb(42_28_21/0.4)] ring-1 ring-line",
        size === "lg" && "py-2 pr-2 pl-5",
        // On a phone the button drops below, so the command isn't cut off.
        "max-sm:flex-wrap max-sm:gap-y-2.5 max-sm:pt-3.5 max-sm:pr-1.5 max-sm:pb-1.5 max-sm:pl-1.5 max-sm:[&>code]:pr-2 max-sm:[&>span:first-child]:pl-2.5",
        stacked &&
          "flex-wrap gap-y-2.5 pt-3.5 pr-1.5 pb-1.5 pl-1.5 [&>code]:px-2.5 [&>code]:text-[13px] [&>span:first-child]:hidden",
        className
      )}
    >
      <span className="font-mono text-ink-soft/70 select-none" aria-hidden>
        $
      </span>
      <code
        ref={code}
        aria-label={label}
        className={cn(
          "no-scrollbar min-w-0 flex-1 overflow-x-auto font-mono text-sm whitespace-nowrap",
          size === "lg" && "sm:text-base"
        )}
      >
        {command}
      </code>
      <button
        type="button"
        onClick={copy}
        className={cn(
          "relative inline-flex h-10 min-w-[6.25rem] shrink-0 items-center justify-center gap-1.5 overflow-hidden rounded-xl bg-coral-deep px-4 text-sm font-semibold text-white transition-[background-color,transform] hover:bg-[#b0321c] active:scale-[0.97]",
          size === "lg" && "h-11",
          "max-sm:w-full",
          stacked && "w-full"
        )}
      >
        <AnimatePresence mode="popLayout" initial={false}>
          <motion.span
            key={state}
            initial={{ y: 14, opacity: 0 }}
            animate={{ y: 0, opacity: 1 }}
            exit={{ y: -14, opacity: 0 }}
            transition={{ duration: 0.18 }}
            className="inline-flex items-center gap-1.5"
          >
            {state === "copied" ? (
              <Check className="size-4" aria-hidden />
            ) : (
              <Copy className="size-4" aria-hidden />
            )}
            {state === "copied"
              ? "Copied"
              : state === "failed"
                ? "Press ⌘C"
                : "Copy"}
          </motion.span>
        </AnimatePresence>
      </button>
      <span className="sr-only" role="status">
        {state === "copied" ? "Copied to your clipboard" : ""}
      </span>
    </div>
  )
}

export function Kbd({ children }: { children: ReactNode }) {
  return (
    <kbd className="inline-flex h-7 min-w-7 items-center justify-center rounded-lg bg-card px-2 font-sans text-sm font-medium shadow-[0_2px_0_rgb(42_28_21/0.18)] ring-1 ring-line">
      {children}
    </kbd>
  )
}

export function InlineCode({ children }: { children: ReactNode }) {
  return (
    <code className="rounded-md bg-ink/[0.07] px-1.5 py-0.5 font-mono text-[0.88em]">
      {children}
    </code>
  )
}

export function GitHubMark({ className }: { className?: string }) {
  return (
    <svg
      viewBox="0 0 16 16"
      className={className}
      fill="currentColor"
      aria-hidden
    >
      <path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27s1.36.09 2 .27c1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.01 8.01 0 0 0 16 8c0-4.42-3.58-8-8-8Z" />
    </svg>
  )
}
