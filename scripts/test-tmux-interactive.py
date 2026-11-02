#!/usr/bin/env python3
"""Interactive regression test for a tmux executable (the one Klaub embeds).

Runs a tmux client on a pseudo-terminal, with a full-screen program in the first window (it
uses the alternate screen, synchronized output, bracketed paste, focus events, the kitty keyboard
protocol, terminal queries, true colour and wide characters, as Claude Code and Codex do) and a
second window beside it. It then switches between the two windows with the prefix key, as
Klaub's Ctrl+B R / Ctrl+B A do, and fails if the tmux server dies at any point.

A tmux 3.8 built with jemalloc 5.4 on Apple Silicon exits unexpectedly under this test; the
same tmux with the system allocator survives it.

Usage: scripts/test-tmux-interactive.py /path/to/tmux [rounds]
"""

import fcntl
import os
import pty
import select
import struct
import subprocess
import sys
import tempfile
import termios
import time
import tty

ROWS, COLS = 40, 120


def fixture() -> None:
    """The full-screen program that runs inside the first tmux window."""
    out = sys.stdout
    fd = sys.stdin.fileno()
    saved = termios.tcgetattr(fd)
    tty.setraw(fd)

    def emit(text: str) -> None:
        out.write(text)
        out.flush()

    emit("\x1b[?1049h\x1b[?2004h\x1b[?1004h\x1b[>1u\x1b[?25l")
    emit("\x1b[c\x1b[>0q\x1b[?u\x1b[?2026$p\x1b]10;?\x1b\\\x1b]11;?\x1b\\")
    frame = 0
    try:
        while True:
            frame += 1
            buf = "\x1b[?2026h\x1b[H"
            for row in range(1, ROWS):
                buf += f"\x1b[{row};1H\x1b[38;2;{(frame * 7 + row * 5) % 256};{(row * 9) % 256};200m"
                buf += f"\x1b[48;2;10;10;{row * 3}m"
                buf += (f"row {row} frame {frame} ✻ \U0001f680 日本語 é " * 6)[:110]
                buf += "\x1b[0m\x1b[K"
            emit(buf + "\x1b[?2026l")
            ready, _, _ = select.select([fd], [], [], 0.03)
            if ready:
                os.read(fd, 4096)
    finally:
        termios.tcsetattr(fd, termios.TCSADRAIN, saved)


def run(tmux: str, rounds: int) -> int:
    socket = f"klaub-interactive-{os.getpid()}"
    base = [tmux, "-L", socket, "-f", "/dev/null"]
    env = dict(os.environ, TERM="xterm-256color", COLORTERM="truecolor")
    env.pop("TMUX", None)

    def alive() -> bool:
        probe = subprocess.run(base + ["has-session", "-t", "t"], capture_output=True, env=env, check=False)
        return probe.returncode == 0

    command = f"{sys.executable} {os.path.abspath(__file__)} --fixture"
    pid, fd = pty.fork()
    if pid == 0:
        os.execvpe(base[0], base + ["new-session", "-s", "t", "-x", str(COLS), "-y", str(ROWS), command], env)
    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLS, 0, 0))

    def drain(seconds: float) -> None:
        deadline = time.time() + seconds
        while time.time() < deadline:
            ready, _, _ = select.select([fd], [], [], 0.05)
            if ready:
                try:
                    if not os.read(fd, 65536):
                        return
                except OSError:
                    return

    try:
        drain(1.5)
        if not alive():
            print("FAIL: the tmux server is not running after the full-screen program started")
            return 1
        subprocess.run(
            base + ["new-window", "-d", "-n", "review", "sh -c 'while :; do sleep 1; done'"],
            check=True,
            env=env,
        )
        for index in range(1, rounds + 1):
            for key, name in ((b"\x02n", "the second window"), (b"\x02p", "the first window")):
                os.write(fd, key)
                drain(0.4)
                if not alive():
                    print(f"FAIL: the tmux server exited while switching to {name} (round {index} of {rounds})")
                    return 1
        print(f"ok: the tmux server survived {rounds} rounds of window switches")
        return 0
    finally:
        subprocess.run(base + ["kill-server"], capture_output=True, env=env, check=False)
        try:
            os.close(fd)
            os.waitpid(pid, 0)
        except OSError:
            pass


def main() -> int:
    if len(sys.argv) >= 2 and sys.argv[1] == "--fixture":
        fixture()
        return 0
    if len(sys.argv) < 2:
        print(__doc__)
        return 2
    with tempfile.TemporaryDirectory() as tmpdir:
        os.environ["TMUX_TMPDIR"] = tmpdir
        return run(sys.argv[1], int(sys.argv[2]) if len(sys.argv) > 2 else 30)


if __name__ == "__main__":
    sys.exit(main())
