# Using Klaub over SSH

SSH is supported on a best-effort basis. The reader and `klaub wrap` are terminal programs that draw through a pseudo-terminal, so they work in an SSH session the way any full-screen terminal program does. What Klaub cannot do from a remote host is reach the machine you are sitting at.

## What works

- `klaub wrap`, `klaub review`, the key chords, the colours and resizing, as long as the terminal you connect from handles a normal full-screen program and `TERM` names it (`xterm-256color`, `tmux-256color`, ...).
- Everything that is a file or a command: the review file, `klaub feedback`, `klaub reviews export`, `klaub flow ...`.
- Preparing feedback in the agent's input, because the agent runs on the same host as Klaub.

## What does not

- **Copy feedback** copies to the clipboard of the host Klaub runs on (`pbcopy`, `wl-copy` or `xclip`), not to your laptop's. Over SSH that is usually nothing you can paste from, or no clipboard program at all. Use **Show file** in the reader, or run `klaub feedback <review-id>` and copy from your terminal.
- Klaub does not use terminal escape sequences (OSC 52) to reach your clipboard. They are not supported everywhere, tmux sits in the way, and sending escape sequences to a terminal the user does not control is exactly what the rest of Klaub avoids.
- Running Klaub inside a tmux you started yourself is refused for `klaub wrap` (a nested tmux), and `klaub review` still works there.

## Checking

`klaub doctor` reports an `ssh` line when it sees `SSH_CONNECTION`, `SSH_TTY` or `SSH_CLIENT`, a `clipboard` line saying which program Copy would use or that there is none, and a `terminal` line that warns on `TERM=dumb`.

## How this is tested

The CLI tests run `klaub doctor` with the SSH variables set and with `TERM=dumb`, and check the output. That covers the detection and the advice. It does **not** open a real SSH connection: no test in this repository runs the reader through `sshd`. If you rely on SSH, the manual check is the one in [the acceptance list](specification.pt-BR.md): `klaub wrap`, switch to the reviewer and back five times, comment, finish and prepare, from a real remote session.
