# Changelog

## 0.2.0 (22 November 2026)

- A new screen: you watch Transclip look the video up and try each place
  the words can come from, then the same screen shows the transcript and
  which source it used. Press `q` at any point to stop; it tidies up as it
  goes.
- Press `s` to save the transcript as a text file in Downloads, or use
  `--save [FOLDER]` to save it somewhere else as well as copying it.
- `--timestamps` puts the time each paragraph starts, like `[12:40]`, in
  front of it.
- If the video has chapters, they become headings in the transcript.
- On videos YouTube has dubbed into other languages, Transclip now takes the
  speaker's own transcript rather than a dub's.
- `--lang` now defaults to your Mac's language instead of English.
- Quicker: YouTube is asked about each video once instead of several times.
- While transcribing on your Mac, the progress bar moves smoothly and says
  how long it took.

## 0.1.1 (18 November 2026)

- New `transclip --report` copies a short report to paste into a GitHub
  issue: your Transclip, macOS, yt-dlp and ffmpeg versions, whether the
  speech model is downloaded, and the last error. Links, video IDs and your
  folder names are left out. When something fails, Transclip mentions it.
- If YouTube says captions were asked for too often (HTTP 429), Transclip
  transcribes on your Mac for the next 30 minutes instead of asking again,
  which would keep the limit going. Videos in other languages still try
  captions, since the Mac's model is English only.
- After that refusal it no longer asks for YouTube's automatic transcript in
  the same run, which would be refused too.
- A refusal like this no longer suggests updating yt-dlp, since that
  wouldn't help.

## 0.1.0 (17 November 2026)

First release, an early preview.
