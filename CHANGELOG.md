# Changelog

## 0.1.1 (18 November 2026)

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
