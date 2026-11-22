/**
 * Every fact and sentence on the site lives here, taken from Transclip's
 * README and source (transclip 0.1.1). Nothing on the page should claim
 * more than this file does.
 */

const REPO = "https://github.com/Cjpowis/transclip"

export const LINKS = {
  repo: REPO,
  discussions: `${REPO}/discussions`,
  issues: `${REPO}/issues`,
  changelog: `${REPO}/blob/main/CHANGELOG.md`,
  licence: `${REPO}/blob/main/LICENSE`,
  author: "https://github.com/Cjpowis",
  homebrew: "https://brew.sh",
  ytdlp: "https://github.com/yt-dlp/yt-dlp",
  ffmpeg: "https://ffmpeg.org",
  parakeetRs: "https://github.com/altunenes/parakeet-rs",
  model: "https://huggingface.co/nvidia/parakeet-unified-en-0.6b",
  modelLicence:
    "https://www.nvidia.com/en-us/agreements/enterprise-software/nvidia-open-model-license/",
} as const

export const INSTALL_COMMAND = "brew install cjpowis/tap/transclip"
export const HOMEBREW_COMMAND =
  '/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"'

export type SourceId = "captions" | "auto" | "mac"

/** The three places the words can come from, in the order Transclip tries them. */
export const SOURCES: {
  id: SourceId
  name: string
  short: string
  blurb: string
}[] = [
  {
    id: "captions",
    name: "The video's own captions",
    short: "Captions",
    blurb: "Written by a person. Instant, and usually the most accurate.",
  },
  {
    id: "auto",
    name: "YouTube's automatic transcript",
    short: "Auto transcript",
    blurb: "Used if it has punctuation. Also instant.",
  },
  {
    id: "mac",
    name: "Listening on your Mac",
    short: "On your Mac",
    blurb:
      "NVIDIA's speech model transcribes the audio on your own computer. A 15-minute video takes about 30 seconds.",
  },
]

export const OPTIONS: { flag: string; does: string }[] = [
  {
    flag: "-t, --transcribe",
    does: "Skip YouTube's text and always transcribe on your Mac",
  },
  {
    flag: "-p, --print",
    does: "Print the transcript instead of showing it on screen",
  },
  {
    flag: "--lang CODE",
    does: "Caption language to look for first, like fr (default: your Mac's language)",
  },
  { flag: "--no-copy", does: "Don't copy to the clipboard" },
  {
    flag: "--text-only",
    does: "Leave the title and link off the copied text",
  },
  {
    flag: "--timestamps",
    does: "Put the time each paragraph starts, like [12:40], in front of it",
  },
  {
    flag: "--save [FOLDER]",
    does: "Also save it as a text file, in Downloads or the folder given",
  },
  { flag: "--delete-model", does: "Delete the downloaded speech model" },
  {
    flag: "--report",
    does: "Copy a short report to paste into a GitHub issue",
  },
]

export const GOOD_TO_KNOW: string[] = [
  "Early preview, version 0.2. For Macs with Apple Silicon (M1 or later).",
  "Live streams, playlists and channel links aren't supported. Give it a single video.",
  "For videos in other languages, Transclip uses YouTube's own transcript. The speech model on your Mac only understands English.",
  "The first time it listens on your Mac, it downloads the speech model once (about 650 MB).",
  "Please respect creators' rights when you use transcripts.",
]
