#!/usr/bin/env python3
"""Record a real Transclip run for the website.

Runs the transclip that is installed on this Mac inside a real terminal
session (a pty), types into it the way a person would, and saves exactly
what comes back, with timing, as an asciicast file the site plays back.
Nothing on screen is written by hand.

    python3 scripts/record-demo.py <name> <youtube-link> [transclip options]

    python3 scripts/record-demo.py captions https://youtu.be/XXXX
    python3 scripts/record-demo.py mac https://youtu.be/XXXX -t

Writes src/recordings/<name>.cast and src/recordings/<name>.json.

Two things are done so recording doesn't disturb this Mac or leak anything:
  * SSH_TTY is set for the session. Transclip then sends the transcript to
    the terminal's clipboard only (the recorder catches it) and leaves the
    Mac's own clipboard alone. What it prints is the same.
  * The shell prompt is plain "~ %", with no user or computer name.
The clipboard escape code itself is cut from the saved file (it is long and
invisible) and kept as text in the .json instead.
"""

import base64
import codecs
import fcntl
import json
import os
import pty
import random
import re
import select
import signal
import struct
import subprocess
import sys
import tempfile
import termios
import threading
import time
from pathlib import Path

COLS, ROWS = 80, 24
OUT = Path(__file__).resolve().parent.parent / "src" / "recordings"
ALT_ON, ALT_OFF = "\x1b[?1049h", "\x1b[?1049l"
OSC52 = re.compile(r"\x1b\]52;c;([A-Za-z0-9+/=]*)(?:\x07|\x1b\\)")
ANSI = re.compile(r"\x1b\[[0-9;?]*[A-Za-z]|\x1b\][^\x07]*\x07|\r")


class Session:
    def __init__(self):
        rc = tempfile.mkdtemp(prefix="transclip-rec-")
        Path(rc, ".zshrc").write_text(
            "PROMPT='%~ %# '\nunsetopt PROMPT_SP\nunset zle_bracketed_paste\n"
        )
        self.pid, self.fd = pty.fork()
        if self.pid == 0:
            fcntl.ioctl(0, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLS, 0, 0))
            os.chdir(os.path.expanduser("~"))
            env = {
                "HOME": os.path.expanduser("~"),
                "PATH": os.environ["PATH"],
                "TERM": "xterm-256color",
                "LANG": "en_GB.UTF-8",
                "ZDOTDIR": rc,
                "SSH_TTY": "recording",
            }
            os.execvpe("zsh", ["zsh", "-i"], env)
        self.start = time.monotonic()
        self.events = []  # (seconds, text)
        self.text = ""
        self.lock = threading.Lock()
        self.alive = True
        threading.Thread(target=self._read, daemon=True).start()

    def _read(self):
        decode = codecs.getincrementaldecoder("utf-8")(errors="replace")
        while self.alive:
            ready, _, _ = select.select([self.fd], [], [], 0.05)
            if not ready:
                continue
            try:
                data = os.read(self.fd, 65536)
            except OSError:
                break
            if not data:
                break
            text = decode.decode(data)
            if text:
                with self.lock:
                    self.events.append((time.monotonic() - self.start, text))
                    self.text += text

    def wait_for(self, needle, timeout, after=0):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            with self.lock:
                at = self.text.find(needle, after)
            if at >= 0:
                return at
            time.sleep(0.05)
        raise TimeoutError(f"never saw {needle!r}")

    def send(self, text):
        os.write(self.fd, text.encode())

    def type(self, text):
        for ch in text:
            self.send(ch)
            time.sleep(random.uniform(0.07, 0.15))

    def close(self):
        self.alive = False
        try:
            os.kill(self.pid, signal.SIGHUP)  # the shell passes it on
            for _ in range(20):
                if os.waitpid(self.pid, os.WNOHANG)[0]:
                    return
                time.sleep(0.1)
            os.kill(self.pid, signal.SIGKILL)
            os.waitpid(self.pid, 0)
        except OSError:
            pass


def record(name, link, options):
    command = " ".join(["transclip", *options])
    s = Session()
    try:
        s.wait_for("% ", 10)
        time.sleep(1.0)
        s.type(command)
        time.sleep(0.5)
        s.send("\r")
        at = s.wait_for("Paste the video link: ", 15)
        time.sleep(0.9)
        s.send(link)  # pasted, so it arrives at once
        time.sleep(0.7)
        s.send("\r")

        # Everything from here to the viewer is Transclip working.
        # Transclip opens its screen straight away and works inside it, so
        # wait for the copy (its clipboard escape code) before reading on.
        s.wait_for(ALT_ON, 60, at)
        s.wait_for("\x1b]52;", 240, at)
        time.sleep(3.2)
        for _ in range(7):  # read down a little
            s.send("\x1b[B")
            time.sleep(0.32)
        time.sleep(0.9)
        s.send(" ")  # a page
        time.sleep(1.8)
        s.send("c")  # copy again
        time.sleep(2.2)
        s.send("q")
        end = s.wait_for(ALT_OFF, 10)
        s.wait_for("% ", 10, end)
        time.sleep(1.6)
    except TimeoutError as problem:
        with s.lock:
            seen = ANSI.sub("", s.text)
        s.close()
        sys.exit(f"Stopped: {problem}. The terminal showed:\n{seen[-1500:]}")
    with s.lock:
        events = list(s.events)
        full = s.text
    s.close()

    # Lift the clipboard escape code out of the stream, noting when it came.
    clipboard, marks, cleaned, carry = None, [], [], ""
    seen = set()
    for t, text in events:
        text = carry + text
        carry = ""
        cut = text.rfind("\x1b]52;")
        if cut >= 0 and not OSC52.search(text, cut):
            text, carry = text[:cut], text[cut:]
        for found in OSC52.finditer(text):
            if clipboard is None:
                clipboard = base64.b64decode(found.group(1)).decode("utf-8", "replace")
            marks.append((t, "copied" if "copied" not in seen else "copied-again"))
            seen.add("copied")
        text = OSC52.sub("", text)
        if ALT_ON in text and "viewer" not in seen:
            marks.append((t, "viewer"))
            seen.add("viewer")
        if ALT_OFF in text and "quit" not in seen:
            marks.append((t, "quit"))
            seen.add("quit")
        if text:
            cleaned.append((t, text))

    plain = ANSI.sub("", full)
    words = re.search(r"Copied to clipboard: ([\d,]+) words", plain)
    version = subprocess.run(
        ["transclip", "--version"], capture_output=True, text=True
    ).stdout.strip()

    OUT.mkdir(parents=True, exist_ok=True)
    header = {"version": 2, "width": COLS, "height": ROWS, "title": command}
    rows = [(t, "o", text) for t, text in cleaned] + [(t, "m", m) for t, m in marks]
    rows.sort(key=lambda r: r[0])
    with open(OUT / f"{name}.cast", "w") as f:
        f.write(json.dumps(header) + "\n")
        for t, kind, data in rows:
            f.write(json.dumps([round(t, 4), kind, data], ensure_ascii=False) + "\n")

    meta = {
        "name": name,
        "command": command,
        "link": link,
        "title": (clipboard or "").split("\n")[0] or None,
        "words": words.group(1) if words else None,
        "seconds": round(cleaned[-1][0], 2) if cleaned else 0,
        "marks": {m: round(t, 2) for t, m in marks},
        "clipboard": clipboard,
        "transclip": version,
    }
    (OUT / f"{name}.json").write_text(json.dumps(meta, indent=2, ensure_ascii=False))
    print(f"{name}: {meta['seconds']}s, title={meta['title']!r}, words={meta['words']}")
    print("  marks:", meta["marks"])
    print("  lines:", [l[:90] for l in plain.split("\n") if l[:1] in "✓•!"][:8])


if __name__ == "__main__":
    if len(sys.argv) < 3:
        sys.exit(__doc__)
    record(sys.argv[1], sys.argv[2], sys.argv[3:])
