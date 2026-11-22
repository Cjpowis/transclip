# Transclip

Get the full transcript of a YouTube video onto your clipboard, ready to
paste into ChatGPT, your notes, or anywhere else.

**Early preview, version 0.3.** For Macs with Apple Silicon (M1 or later)
and Windows 10 or 11 PCs. Free and open source.

## Install

### On a Mac

1. Open **Terminal**: press ⌘ and Space, type `Terminal`, press Return.
2. If you don't have Homebrew yet, paste this and press Return (it's the
   one line from [brew.sh](https://brew.sh)):

   ```
   /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
   ```

   It asks for your Mac password; nothing shows as you type, which is
   normal. When it finishes it prints **Next steps** with a couple of lines
   to paste. Paste them, or Terminal won't find `brew`.
3. Paste this and press Return:

   ```
   brew install cjpowis/tap/transclip
   ```

   This also installs **yt-dlp** and **ffmpeg**, which Transclip uses, and
   NVIDIA's speech model (about 650 MB).

### On Windows

1. Download
   [**Transclip-Setup.exe**](https://github.com/Cjpowis/transclip/releases/latest/download/Transclip-Setup.exe).
2. Open it. The first time, Windows may say it **protected your PC**,
   because Transclip isn't signed with a paid certificate: click
   **More info**, then **Run anyway**.
3. Click **Install**. It installs Transclip, **yt-dlp** and **ffmpeg**
   (with winget, Windows' own installer) and NVIDIA's speech model (about
   650 MB), so it takes a few minutes.

## Use

Type `transclip` and press Return, then paste a YouTube link and press Return.
On Windows you can also open **Transclip** from the Start menu or the desktop.

You watch it work on one screen: it looks the video up, tries each place the
words can come from, and shows which one it used. The transcript is copied to
your clipboard straight away and shown below. Press `c` to copy it again, `s`
to save it as a text file in Downloads, `q` to quit.

If the video has chapters, they become headings in the transcript.

## Where the text comes from

Transclip takes the first of these that works:

1. **The video's human-written captions.** Instant, and usually the most
   accurate.
2. **YouTube's automatic transcript** in the video's own language, if it has
   punctuation. On videos YouTube has dubbed into other languages, it takes
   the speaker's own transcript, not a dub's.
3. **Transcribing the audio on your Mac** with NVIDIA's speech model
   (English only). The first time, it downloads the model once (about
   650 MB). After that, a 15-minute video takes about 30 seconds.

Want the most accurate text for names and numbers? Use `transclip -t` to go
straight to step 3.

If YouTube says captions were asked for too often, Transclip goes straight to
step 3 for the next 30 minutes rather than asking again. (Videos in other
languages still try captions, since step 3 is English only.)

## Over SSH, to your phone

Run Transclip over SSH from a phone app such as Termius and the transcript
lands on your **phone's** clipboard, not the Mac's.

## Options

| Option | What it does |
| --- | --- |
| `-t`, `--transcribe` | Skip YouTube's text and always transcribe on your Mac |
| `-p`, `--print` | Print the transcript instead of showing it on screen |
| `--lang CODE` | Caption language to look for first, like `fr` (default: your Mac's language) |
| `--no-copy` | Don't copy to the clipboard |
| `--text-only` | Leave the title and link off the copied text |
| `--timestamps` | Put the time each paragraph starts, like `[12:40]`, in front of it |
| `--save [FOLDER]` | Also save it as a text file, in Downloads or the folder given (made if it's missing) |
| `--delete-model` | Delete the downloaded speech model |
| `--report` | Copy a short report to paste into a GitHub issue |

## Keep yt-dlp up to date

YouTube changes often, and yt-dlp keeps up within days. If a video won't
download, Transclip tells you when your yt-dlp is out of date and how to
update it. For the most reliable results, switch to yt-dlp's latest code:

```
brew reinstall --HEAD yt-dlp
```

and update it now and then with `brew upgrade --fetch-HEAD yt-dlp`. On
Windows, update it with `winget upgrade yt-dlp.yt-dlp`.

## Good to know

- Live streams, playlists and channel links aren't supported. Give it a
  single video.
- For videos in other languages, Transclip uses YouTube's own transcript;
  the speech model on your computer only understands English.
- Nothing is saved except the speech model, the last error message (for
  `--report`), a tiny note of when to try captions again if YouTube says
  they were asked for too often, and any transcript you choose to save.
  Downloads go in a temporary folder that's deleted afterwards, even if you
  press `q` or Ctrl-C partway through.
- No account, no cloud, no tracking. Everything runs on your computer.
- Please respect creators' rights when you use transcripts.

## Uninstall

On a Mac:

```
brew uninstall transclip
```

On Windows: **Settings**, **Apps**, **Transclip**, **Uninstall**.

Either way the speech model goes too. yt-dlp and ffmpeg stay, as other
programs may use them.

## Questions or problems

Ask a question in
[Discussions](https://github.com/Cjpowis/transclip/discussions), or report a
problem in [Issues](https://github.com/Cjpowis/transclip/issues).

For a problem, run `transclip --report` and paste what it copies into the
issue. It lists your Transclip, macOS or Windows, yt-dlp and ffmpeg versions
and the last error, and leaves out links, video IDs and your folder names.

What's changed in each version is in [CHANGELOG.md](CHANGELOG.md).

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
