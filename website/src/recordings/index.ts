/**
 * Real recordings of Transclip, made by scripts/record-demo.py.
 * Each .cast file is exactly what the program printed, with timing; the
 * .json beside it holds what landed on the clipboard and when.
 * To replace one, run the script again with another link.
 */
import type { SourceId } from "@/content/transclip"

import autoCast from "./auto.cast?url"
import autoMeta from "./auto.json"
import captionsCast from "./captions.cast?url"
import captionsMeta from "./captions.json"
import macCast from "./mac.cast?url"
import macMeta from "./mac.json"

export type Recording = {
  id: SourceId
  /** Label on the picker. */
  pick: string
  /** What was typed to start it. */
  command: string
  cast: string
  title: string
  link: string
  words: string
  seconds: number
  /** When the transcript reached the clipboard, in seconds. */
  copiedAt: number
  /** The clipboard text, split into its blank-line-separated blocks. */
  clipboard: string[]
  version: string
}

type Meta = typeof captionsMeta

function recording(
  id: SourceId,
  pick: string,
  cast: string,
  meta: Meta
): Recording {
  return {
    id,
    pick,
    cast,
    command: meta.command,
    title: meta.title ?? "",
    link: meta.link,
    words: meta.words ?? "",
    seconds: meta.seconds,
    copiedAt: meta.marks.copied,
    clipboard: (meta.clipboard ?? "").split(/\n{2,}/),
    version: meta.transclip.replace("transclip ", ""),
  }
}

export const RECORDINGS: Recording[] = [
  recording("captions", "Captions", captionsCast, captionsMeta),
  recording("auto", "Only an auto transcript", autoCast, autoMeta),
  recording("mac", "Listening on the Mac", macCast, macMeta),
]

/** The first few words a recording's transcript really starts with. */
export function opening(id: SourceId, words: number): string {
  const rec = RECORDINGS.find((r) => r.id === id)
  const body = rec?.clipboard.find(
    (block, i) => i > 0 && !block.startsWith("#") && !block.startsWith("http")
  )
  return (body ?? "").split(/\s+/).slice(0, words).join(" ")
}
