# Windows support

Status: investigation. No target version. Nothing here is a commitment to ship Windows support.

## What the runtime depends on

The embedded runtime is tmux. `klaub wrap` starts its own tmux server on a private socket and records pane ids against it ([ADR 0008](adr/0008-one-private-tmux-server-per-session.md), refining [ADR 0004](adr/0004-embedded-verified-tmux.md)). The binary that gets embedded is produced by `scripts/build-tmux.sh` for one target triple at a time, and that script builds the upstream tmux sources. tmux is not native on Windows, so the embedded runtime cannot ship there as it stands. No Windows tmux asset exists, and the CI that builds the assets targets Linux and macOS.

## WSL2 is not the same thing as Windows support

WSL2 can run the Linux binary, including its embedded tmux. That does not make the Windows side work. The tmux session lives inside the Linux virtual machine, and the terminal the user is looking at lives on the Windows side. Pane identity, focus switching and paste are all operations against the tmux server, so each of them would need to be re-validated across that boundary rather than assumed to behave as it does when the terminal and the server are on the same machine.

## Paths and the home directory

The two sides do not share a path space. A repository checked out on the Windows side is visible to WSL2 under `/mnt`, with different path spelling, and the home directory that holds Klaub's state is the Linux one, not the Windows one. Anything that stores or compares paths, or that looks for configuration under the home directory, would have to say which side it means.

## What `klaub doctor` does about it

Under WSL, `klaub doctor` prints a `wsl` warning that this is not a supported target, and a second sentence when the current directory is on the Windows file system (`/mnt/<drive>`), where git is slow and paths differ from the Linux side. It detects WSL from the kernel release or `WSL_DISTRO_NAME`. That is all it does: it reports the situation and claims nothing about it.

## Before support could be claimed

All of the following would have to be true, and shown:

- a tmux that Klaub can verify and embed actually runs in the environment being claimed, or the design no longer requires one;
- pane identity and paste behave correctly when the terminal and the tmux server are not on the same machine, which is the WSL2 case;
- paths and the home directory are unambiguous between the two sides;
- the claims are covered by the same kind of automated gate the Linux and macOS harnesses already have, not by a manual check.
