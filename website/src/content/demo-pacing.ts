import type { SourceId } from "@/content/transclip"

/**
 * How the recording at the top of the page behaves.
 *
 *  - `startWith` is the recording that plays when the page opens.
 *    "captions" is the quickest (about 15 seconds).
 *    "auto" is the middle path, about the same length.
 *    "mac" is the longest and most impressive: about 40 seconds, most of
 *    it the Mac listening to nine minutes of audio.
 *
 *  - `speed` is playback speed. 1 is exactly as it happened, and the page
 *    says so. 1.5 or 2 is brisker, but it stops being "real speed".
 */
export const DEMO_PACING: { startWith: SourceId; speed: number } = {
  startWith: "captions",
  speed: 1,
}

/** Milliseconds each state stays lit in "Where the words come from". */
export const TIMING = {
  explainerHold: 5600,
  explainerStep: 850,
} as const
