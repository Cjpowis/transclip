# The Transclip website

The page at [cjpowis.github.io/transclip](https://cjpowis.github.io/transclip/).
It's a small React site, built with Vite, Tailwind and Motion, and the build
is what GitHub Pages serves from `../docs`.

```
cd website
npm install
npm run dev      # a preview at http://localhost:5173
npm run build    # writes the finished site into ../docs
```

Commit `docs/` after building, as GitHub Pages serves that folder as it is.

## What's on it

- All the wording and facts live in `src/content/transclip.ts`, taken from
  the README. Change them there.
- The Terminal at the top is real: recordings of Transclip running, in
  `src/recordings`, played back with
  [asciinema-player](https://github.com/asciinema/asciinema-player). Nothing
  in that window is drawn by the site.

## Recording a new demo

With Transclip installed, run for example:

```
python3 scripts/record-demo.py captions "https://youtu.be/VIDEO"
python3 scripts/record-demo.py mac "https://youtu.be/VIDEO" -t
```

The names used by the page are `captions`, `auto` and `mac`. The script runs
Transclip in a real terminal session, types into it like a person, and saves
what it printed with its timing. It leaves your Mac's clipboard alone and
uses a plain `~ %` prompt, so no user or computer name is recorded.

The current recordings use NASA videos, which are public domain.
