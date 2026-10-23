# Research: controlling tmux from the Tauri app on macOS

Date: 2026-10-22. Status: a spike is built, see [The spike](#the-spike).

Goal: from the desktop app, create tmux sessions, send keys to them, and read
their status. Distribution: signed with an Apple Developer ID and notarized,
not through the Mac App Store.

## Summary

Tauri can do this. Tauri does not sandbox the app: the Rust backend is a normal
native macOS process and can run any program, including `tmux`. The only thing
that blocks it is Apple's **App Sandbox**, which is required for the Mac App
Store but not for Developer ID signing and notarization.

## Experiment

A small Rust program ran the same calls a Tauri command would (source below).
It created a session, sent `echo hello-from-rust; pwd` + Enter, read the
screen, listed sessions, read the pane's running command, and killed the
session. It ran under four setups:

| # | Setup | Result |
|---|---|---|
| 1 | Plain binary (like `tauri dev`) | ✅ Every step worked |
| 2 | Same, with the PATH a Finder or Dock launch gets (`/usr/bin:/bin:/usr/sbin:/sbin`) | ❌ `No such file or directory`: `tmux` isn't on that PATH |
| 3 | Signed with hardened runtime (`codesign --options runtime`), the configuration notarization requires | ✅ Every step worked |
| 4 | Real `.app` bundle with `com.apple.security.app-sandbox` (what the Mac App Store requires) | ❌ `Operation not permitted` (EPERM), even with the full path `/opt/homebrew/bin/tmux` |

The tests used ad-hoc signing (`codesign -s -`). A real Developer ID signature
applies the same hardened runtime and sandbox rules.

A sandboxed binary that isn't inside an `.app` bundle doesn't run at all: it
exits with code 133 (SIGTRAP). Setup 4 therefore used a minimal bundle with an
`Info.plist`.

## What this means for the app

- **Tauri's permissions don't limit the Rust side.** The files in
  `src-tauri/capabilities/` only control which commands the web page can call.
  A `#[tauri::command]` that runs `std::process::Command::new("tmux")` is
  ordinary native code. Put the tmux logic in Rust commands like
  `create_tmux_session`, `send_tmux_keys` and `get_tmux_status`. Don't give the
  web page general shell access through the shell plugin.
- **Never turn on the App Sandbox.** Programs the app starts inherit its
  sandbox. Even a copy of tmux shipped inside the app would run sandboxed. It
  couldn't reach the user's normal tmux server socket in `/tmp/tmux-<uid>/`, so
  the app would only see a private, empty tmux. (Inferred from Apple's
  documentation, not tested.) That's the main reason this app has to ship
  outside the Mac App Store.
- **Fix the PATH at startup.** When the app is launched from Finder it doesn't
  get the user's shell PATH, so it can't find `tmux`. Asking the login shell
  works: `zsh -lc 'command -v tmux'` returned `/opt/homebrew/bin/tmux` with the
  Finder-like PATH. Checking `/opt/homebrew/bin` (Apple Silicon) and
  `/usr/local/bin` (Intel) directly is a simpler fallback.
- **The app shares the user's tmux server.** The test session showed up next to
  the user's other sessions in `tmux ls`, so the user can `tmux attach` to
  anything the app creates. Give the app's sessions a name prefix so it never
  touches the user's other sessions.
- **Reading status:**
  - `tmux list-sessions -F '#{session_name} attached=#{session_attached} windows=#{session_windows}'`
    gives session names and whether they're attached.
  - `tmux display-message -p -t NAME '#{pane_current_command} pid=#{pane_pid}'`
    shows what's running in a session.
  - `tmux capture-pane -p -t NAME` returns the screen text.
- **Wait before reading the screen.** In the test, the screen read 500ms after
  sending keys only showed the typed command, not its output, because the shell
  hadn't finished starting. To check that a command finished, poll
  `capture-pane` or `#{pane_current_command}`. Don't rely on a fixed delay.

## Not tested yet

- **Opening a terminal window attached to a session.** The simplest route is
  running `open -a Ghostty --args -e tmux attach -t NAME` from Rust. Scripting
  Terminal or iTerm with AppleScript should also work under a Developer ID, but
  it needs extra setup:
  - the `com.apple.security.automation.apple-events` entitlement
  - an `NSAppleEventsUsageDescription` entry in `Info.plist`
  - a one-time macOS permission prompt for the user
- **A real Tauri build.** These tests used a standalone Rust program signed the
  same way a Tauri app would be. Next step: add the commands in `src-tauri/`,
  then build, sign and notarize the app.

## The spike

Built 2026-10-23: the tmux example page (see the README). What it showed:

- **Creating and sending a command works.** The app runs `tmux new-session -d
  -s NAME -c DIRECTORY -- $SHELL -l`, then `send-keys -l COMMAND` and `Enter`. The
  login shell gets the user's full PATH even when the app has Finder's.
  Keys sent before the shell is ready wait in the terminal and still run.
- **Opening a window works in every terminal tried.**
  - Ghostty 1.3: AppleScript `new window with configuration` with `command`
    set. This avoids the "Allow Ghostty to execute" prompt that
    `open -na Ghostty.app --args -e ...` shows every time.
  - WezTerm: `open -n -b com.github.wez.wezterm --args start -- tmux attach-session ...`.
  - Terminal (comes with macOS): AppleScript `do script`. When Terminal isn't
    running it opens a window of its own on launch, so the command goes
    `in window 1` instead of opening a second window. When the session is
    detached, the window stays open at a shell prompt (Ghostty closes it).
  - iTerm2: AppleScript `create window with default profile command`.
    iTerm2 wasn't installed, so this is untested.
  - kitty: `open -n -b net.kovidgoyal.kitty --args tmux attach-session ...`.
    kitty wasn't installed, so this is untested.
- **Terminal and iTerm2 find windows by tty.** `tmux list-clients -F
  '#{client_tty}'` gives the ttys of the terminals attached to a session.
  Terminal tabs and iTerm2 sessions have a `tty` property in AppleScript, so
  the match is exact.
  - Terminal: `set selected tab of w to t`, `set index of w to 1`, then
    `activate`.
  - iTerm2: `select` the window, tab and session. Untested.
- **Ghostty and WezTerm find windows by title.** They don't expose ttys. Before opening, the app
  sets the session's `set-titles on` and `set-titles-string "tmux:#S"`.
  These are session options, so the user's other sessions are unaffected. Every
  terminal attached to the session then has the title `tmux:NAME`.
  - Ghostty: `every terminal whose name is "tmux:NAME"`, then `focus`.
  - WezTerm: `wezterm cli list --format json`, then `wezterm cli activate-pane`
    and `open -b` to bring the app to the front. `wezterm cli` only sees the
    newest WezTerm process, and `open -n` starts one per window, so focus can
    miss windows in older processes.
  - kitty: needs `allow_remote_control` in the user's kitty config, so it
    isn't tried.
- **Targets must be `=NAME:`.** Plain `NAME` also matches sessions that start
  with it. `=NAME` alone is rejected by `set-option` ("no such session").
- **Apple events need permission.** macOS asks once per terminal app.
  `src-tauri/Info.plist` has the `NSAppleEventsUsageDescription` text. A
  hardened runtime build also needs the
  `com.apple.security.automation.apple-events` entitlement (not added yet).

Verified by running the Rust functions behind the page's commands against the
real tmux server, with Ghostty 1.3.1, WezTerm 20240203 and Terminal on macOS
26:
- create opened a window
- creating the same name again was refused
- Open on an attached session focused its window
- Open after a detach opened a new window
- the command's output showed in the pane
- starting Terminal from scratch opened one window, not two

In one early run, Terminal's window had not attached to tmux 6 seconds after
it was launched from scratch. It didn't happen again in 5 more fresh
launches, which attached within 2 seconds.

The page itself wasn't click-tested.

## Detecting what runs in a pane

Research 2026-10-23, probed against the user's real tmux panes, several of
them running Claude Code. The tmux example page now shows, for every pane:
- the foreground program and whether it's a shell at a prompt (from `ps`)
- the Claude Code status (from `~/.claude/sessions`)
- the git status of its directory

Hooks are left for later. Things learned building it:

- **Field separators:** tmux prints control characters in `-F` output
  escaped, like `\037`, so a unit separator can't split fields. Tabs come
  through as they are.
- **Pane ids are per server:** `%18` on one tmux server isn't `%18` on
  another. Claude Code's `tmux` field doesn't say which server, so the page
  matches on the session name and the pane id. On this machine one Claude
  Code session ran on a separate server (`tmux -L nova`) and is listed as
  "another tmux server".
- **One status read takes about 0.8 seconds** with 7 sessions and 22 panes:
  one `ps -A`, one `tmux list-panes -a`, and `git status` once per distinct
  pane directory, all at the same time. Each `git status` has a 3-second
  timeout.
- **`git --no-optional-locks`** keeps the background `git status` from
  taking the index lock, which would otherwise make the user's own git
  commands fail with `index.lock exists`.

### Any program

- **tmux formats:** `tmux list-panes -a -F ...` gives everything below in one
  call.
  - `#{pane_current_command}`: the name of the program that has control of
    the pane's terminal. It shows `zsh` at an idle prompt, and `nvim` or
    `beam.smp` while those run. Claude Code shows as its version number, like
    `2.1.309`, because its binary is `~/.local/share/claude/versions/2.1.309`.
  - `#{pane_pid}`: the pane's first process, usually the shell.
  - `#{pane_tty}`: the pane's tty.
  - `#{pane_current_path}`: the pane's working directory.
  - `#{pane_dead}`: whether the pane's program has exited.
  - `#{window_activity}`: when the window last had output.
  - `#{pane_title}`: the title the program set.
- **ps:** `ps -t ttysNNN -o pid,ppid,pgid,tpgid,stat,%cpu,comm` lists the
  processes on a pane's tty. The one whose `STAT` has `+` (its PGID equals
  `TPGID`) is the foreground job. If that's the shell, the pane is idle at a
  prompt. If it's anything else, a command is running. `%cpu` tells a busy
  program from one waiting for input.
  - Processes a program starts without a tty don't show up here. Claude Code's
    tool commands are like that. Walk the tree with `pgrep -P <pid>` for those.
- **Output activity:**
  - Compare `capture-pane -p` snapshots, or watch `#{window_activity}`.
  - tmux's `monitor-activity` and `monitor-silence` options flag windows that
    had output, or none for N seconds.
  - `tmux -C` (control mode) streams `%output` events for every pane. This
    allows push updates instead of polling.
- **Exit:** a session started with a command (not a shell) can use
  `remain-on-exit on`. Then `#{pane_dead}` and `#{pane_dead_status}` give the
  exit code. `set-hook pane-exited` can run a command when a pane exits.

### Claude Code

The program doesn't exit between prompts, so the process alone says "Claude is
running", not whether it's working or waiting. Options, from most to least
reliable:

1. **Hooks (documented).** Hooks in `~/.claude/settings.json` (or the
   project's) run a command or POST to a URL. Each call gets JSON with
   `session_id`, `cwd`, `transcript_path` and `hook_event_name`. These events
   map to states:
   - `UserPromptSubmit` → working.
   - `Stop` → finished its turn, waiting for the user.
   - `Notification` with matcher `permission_prompt` → waiting for a
     permission decision.
   - `Notification` with matcher `idle_prompt` → idle for a while.
   - `SessionStart` and `SessionEnd` → the session started or ended.

   The desktop app could install a hook that POSTs to a local HTTP port, or
   appends to a file the app watches. That gives push updates with no polling.
   The hook can also run `tmux display -p '#{session_name}:#{pane_id}'`, or
   read `$TMUX_PANE`, to say which pane it came from.
2. **`~/.claude/sessions/<pid>.json` (undocumented, may change).** There's one
   file per running Claude Code process. Seen with Claude Code 2.1.286–2.1.309.
   - Fields: `pid`, `sessionId`, `cwd`, `name`, `version`, `kind`
     (`interactive`), `status`, `updatedAt` and `statusUpdatedAt`.
   - `status` values seen: `busy`, `idle` and `shell`.
   - The `tmux` field gives the exact pane, like
     `"starter_pack_worktree-macos-integration:@33.%60"` (session, `@window`,
     `%pane`). It matches tmux's `#{pane_id}`, so no process tree matching is
     needed.
   - Files of processes that have exited may linger, so check that the `pid`
     is alive (`kill -0`).
   - Reading this needs no setup, so it's the quickest way to a status column.
3. **Transcripts.** `~/.claude/projects/<cwd with / replaced by ->/<session
   id>.jsonl` holds the conversation. Its last entry tells whether Claude is
   mid tool call or done, and gives the topic. Documented as possibly lagging.
4. **Pane title.** Claude Code sets it to `✳ <topic>`. It stayed `✳` while
   busy, so it's good for a label, not a status.
5. **Screen scraping.** `capture-pane -p` and look for the prompt box or
   permission dialog. This is fragile across versions, so use it as a last
   resort.

### Other agent CLIs

None were installed here, so none of this was checked. The same layers apply:
- process name via tmux and `ps`
- the tool's own hook or notify setting, which Codex CLI has
- its session logs under its config folder, which differ per tool

A small adapter per tool that maps its signal to a shared set of states
(working, waiting for input, waiting for permission, idle, exited) would keep
the UI the same for every tool.

## Reproducing

```rust
// src/main.rs of a scratch `cargo init` project
use std::process::Command;

fn tmux(args: &[&str]) -> String {
    let bin = std::env::var("TMUX_BIN").unwrap_or("tmux".into());
    match Command::new(bin).args(args).output() {
        Ok(o) => format!("exit={} stdout={:?} stderr={:?}", o.status,
            String::from_utf8_lossy(&o.stdout).trim(), String::from_utf8_lossy(&o.stderr).trim()),
        Err(e) => format!("SPAWN FAILED: {e}"),
    }
}

fn main() {
    let s = "tauri-exp";
    println!("new-session: {}", tmux(&["new-session", "-d", "-s", s, "-x", "120", "-y", "30"]));
    println!("send-keys:   {}", tmux(&["send-keys", "-t", s, "echo hello-from-rust; pwd", "Enter"]));
    std::thread::sleep(std::time::Duration::from_millis(500));
    println!("capture:     {}", tmux(&["capture-pane", "-p", "-t", s]));
    println!("status:      {}", tmux(&["list-sessions", "-F", "#{session_name} attached=#{session_attached}"]));
    println!("pane cmd:    {}", tmux(&["display-message", "-p", "-t", s, "#{pane_current_command} pid=#{pane_pid}"]));
    println!("kill:        {}", tmux(&["kill-session", "-t", s]));
}
```

```sh
# 2. Finder-like PATH
env -i HOME=$HOME PATH=/usr/bin:/bin:/usr/sbin:/sbin ./target/debug/tmuxexp

# 3. Hardened runtime
cp target/debug/tmuxexp hardened
codesign -s - -f --options runtime -i com.example.tmuxexp hardened && ./hardened

# 4. App Sandbox: put the binary in Foo.app/Contents/MacOS/ next to an
# Info.plist (CFBundleIdentifier, CFBundleExecutable, CFBundlePackageType=APPL),
# then sign with an entitlements file containing
# com.apple.security.app-sandbox = true
codesign -s - -f --options runtime --entitlements sandbox.entitlements Foo.app
codesign -d --entitlements - Foo.app   # confirm app-sandbox is present
```

## Sources

- [Apple: Hardened runtime](https://developer.apple.com/documentation/security/hardened-runtime.md)
- [Eclectic Light: Notarization and the hardened runtime](https://eclecticlight.co/2021/01/07/notarization-the-hardened-runtime/)
- [Xojo: macOS apps from sandboxing to notarization](https://blog.xojo.com/2024/08/22/macos-apps-from-sandboxing-to-notarization-the-basics/)
- [ibl.ai: separate sandboxed App Store build and non-sandboxed Developer ID build](https://ibl.ai/developer/os/macos-builds)
- [Apple Forums: How are child processes sandboxed?](https://developer.apple.com/forums/thread/123873)
- [Apple Forums: Can't run ps/pgrep in sandbox](https://developer.apple.com/forums/thread/776473)
- [opcode issue: PATH missing when launched from Finder](https://github.com/winfunc/opcode/issues/58)
- [fix-path](https://openapps.pro/packages/fix-path)
