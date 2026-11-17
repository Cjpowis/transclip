# Transclip

Get the full transcript of a YouTube video onto your clipboard, ready to
paste into ChatGPT, your notes, or anywhere else.

**Early preview, version 0.1.** For Macs with Apple Silicon (M1 or later).
Free and open source.

## Install

1. Open **Terminal**: press ⌘ and Space, type `Terminal`, press Return.
2. If you don't have Homebrew yet, install it with the one line on
   [brew.sh](https://brew.sh).
3. Paste this and press Return:

   ```
   brew install cjpowis/tap/transclip
   ```

   This also installs **yt-dlp** and **ffmpeg**, which Transclip uses.

## Use

Type `transclip` and press Return, then paste a YouTube link and press Return.

The transcript is copied to your clipboard straight away and shown on screen.
Press `c` to copy it again, `q` to quit.

## Where the text comes from

Transclip takes the first of these that works:

1. **The video's human-written captions.** Instant, and usually the most
   accurate.
2. **YouTube's automatic transcript**, if it has punctuation.
3. **Transcribing the audio on your Mac** with NVIDIA's speech model
   (English only). The first time, it downloads the model once (about
   650 MB). After that, a 15-minute video takes about 30 seconds.

Want the most accurate text for names and numbers? Use `transclip -t` to go
straight to step 3.

## Over SSH, to your phone

Run Transclip over SSH from a phone app such as Termius and the transcript
lands on your **phone's** clipboard, not the Mac's.

## Options

| Option | What it does |
| --- | --- |
| `-t`, `--transcribe` | Skip YouTube's text and always transcribe on your Mac |
| `-p`, `--print` | Print the transcript instead of showing it on screen |
| `--lang CODE` | Caption language to look for first (default `en`) |
| `--no-copy` | Don't copy to the clipboard |
| `--text-only` | Leave the title and link off the copied text |
| `--delete-model` | Delete the downloaded speech model |

## Keep yt-dlp up to date

YouTube changes often, and yt-dlp keeps up within days. If a video won't
download, Transclip tells you when your yt-dlp is out of date and how to
update it. For the most reliable results, switch to yt-dlp's latest code:

```
brew reinstall --HEAD yt-dlp
```

and update it now and then with `brew upgrade --fetch-HEAD yt-dlp`.

## Good to know

- Live streams, playlists and channel links aren't supported. Give it a
  single video.
- For videos in other languages, Transclip uses YouTube's own transcript;
  the speech model on your Mac only understands English.
- Nothing is saved except the speech model. Downloads go in a temporary
  folder that's deleted afterwards, even if you press Ctrl-C.
- No account, no cloud, no tracking. Everything runs on your Mac.
- Please respect creators' rights when you use transcripts.

## Uninstall

```
transclip --delete-model
brew uninstall transclip
```

## Credits

- [yt-dlp](https://github.com/yt-dlp/yt-dlp) and [FFmpeg](https://ffmpeg.org)
  do the downloading and audio conversion.
- Speech recognition is NVIDIA's
  [parakeet-unified-en-0.6b](https://huggingface.co/nvidia/parakeet-unified-en-0.6b),
  run with [parakeet-rs](https://github.com/altunenes/parakeet-rs). Transclip
  doesn't include the model: the first time it's needed, your Mac downloads
  the ONNX export by
  [bobNight](https://huggingface.co/bobNight/parakeet-unified-en-0.6b-onnx)
  from Hugging Face, and checks every file's fingerprint. The model is
  licensed by NVIDIA Corporation under the
  [NVIDIA Open Model License](https://www.nvidia.com/en-us/agreements/enterprise-software/nvidia-open-model-license/),
  which applies to your use of it.

## Licence

MIT. See [LICENSE](LICENSE).
