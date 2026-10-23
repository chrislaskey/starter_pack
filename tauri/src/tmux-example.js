// The tmux example page. The Rust side guesses the settings and does the
// work (see src-tauri/src/tmux_example.rs); this page shows the guesses in a
// form the user can correct and passes them along with every action.

import { log } from "./log.js";

const { invoke } = window.__TAURI__.core;

const el = (id) => document.getElementById(id);

const REFRESH_INTERVAL_MS = 5000;

const OUTCOMES = {
  focused: "Brought its window to the front",
  opened: "Opened a new window",
};

const CLAUDE_CODE_PILLS = { idle: "pill-good", busy: "pill-busy" };

// Creates an element with a class, text and title, and appends children.
function make(tag, { className, text, title } = {}, ...children) {
  const element = document.createElement(tag);
  if (className) element.className = className;
  if (text !== undefined) element.textContent = text;
  if (title) element.title = title;
  element.append(...children);
  return element;
}

function pill(text, kind = "", title = "") {
  return make("span", { className: `pill ${kind}`, text, title });
}

let homeDirectory;

// Shows the end of a long path, where the interesting part is, with ~ for
// the home directory.
function path(text) {
  const short = homeDirectory && text.startsWith(homeDirectory) ? `~${text.slice(homeDirectory.length)}` : text;
  const span = make("span", { text: short });
  span.dir = "ltr";
  return make("div", { className: "path", title: text }, span);
}

function timeAgo(milliseconds) {
  if (!milliseconds) return "";
  const seconds = Math.max(0, Math.round((Date.now() - milliseconds) / 1000));
  if (seconds < 60) return `${seconds}s ago`;
  if (seconds < 3600) return `${Math.round(seconds / 60)}m ago`;
  if (seconds < 86400) return `${Math.round(seconds / 3600)}h ago`;
  return `${Math.round(seconds / 86400)}d ago`;
}

// Settings

function readSettings() {
  const form = new FormData(el("settings"));
  return {
    shell: form.get("shell").trim(),
    terminal_app: form.get("terminal_app"),
    tmux_path: form.get("tmux_path").trim(),
  };
}

function renderSettingsSummary() {
  const settings = readSettings();
  const select = el("settings").elements.terminal_app;
  const terminalApp = select.selectedOptions[0]?.dataset.name ?? settings.terminal_app;
  el("settings-summary").textContent = [settings.shell, terminalApp, settings.tmux_path].join(" · ");
}

function renderSettings({ settings, terminal_apps, directory, home_directory }) {
  homeDirectory = home_directory;
  el("new-session").elements.directory.value = directory;

  const form = el("settings");
  form.elements.shell.value = settings.shell;
  form.elements.tmux_path.value = settings.tmux_path;

  const select = form.elements.terminal_app;
  select.replaceChildren(
    ...terminal_apps.map((app) => {
      const option = make("option", {
        text: app.path ? app.name : `${app.name} (not found)`,
        title: app.path ?? "",
      });
      option.value = app.id;
      option.dataset.name = app.name;
      return option;
    }),
  );
  select.value = settings.terminal_app;
  renderSettingsSummary();
}

// Sessions

function renderSession(session) {
  const attached = session.attached_clients > 0;

  const attachCommand = `tmux attach -t ${session.name}`;
  const code = make("code", { text: attachCommand, title: "Paste into any terminal to attach" });
  const copy = make("button", { className: "secondary small", text: "Copy" });
  copy.addEventListener("click", () => copyText(attachCommand, code));
  const open = make("button", { className: "small", text: "Open" });
  open.addEventListener("click", () => openSession(session.name));

  const header = make(
    "div",
    { className: "session-header" },
    make("span", { className: "name", text: session.name }),
    pill(attached ? "attached" : "detached", attached ? "pill-good" : ""),
    make("div", { className: "attach" }, code, copy, open),
  );

  return make("div", { className: "card" }, header, ...session.windows.flatMap(renderWindow));
}

// tmux names windows after their program by default, which for Claude Code
// is its version number, so use the readable program name then.
function windowName(tmuxWindow) {
  const pane = tmuxWindow.panes.find((pane) => pane.active) ?? tmuxWindow.panes[0];
  if (pane && tmuxWindow.name === pane.current_command && pane.foreground_program) {
    return pane.foreground_program;
  }
  return tmuxWindow.name;
}

function renderWindow(tmuxWindow) {
  const label = make("div", {
    className: "window-label",
    text: `Window ${tmuxWindow.index} · ${windowName(tmuxWindow)}${tmuxWindow.active ? " · current" : ""}`,
  });
  return [label, ...tmuxWindow.panes.map((pane) => renderPane(tmuxWindow, pane))];
}

function renderPane(tmuxWindow, pane) {
  const location = make("span", {
    className: `pane-location${tmuxWindow.active && pane.active ? " active" : ""}`,
    text: `${tmuxWindow.index}.${pane.index}`,
    title: `Pane ${pane.id}${pane.active ? ", the window's current pane" : ""}`,
  });

  return make(
    "div",
    { className: "pane" },
    location,
    renderPaneState(pane),
    pane.claude_code ? renderClaudeCodePill(pane.claude_code) : make("span"),
    path(pane.current_path),
    pane.git ? renderGitStatus(pane.git) : make("span"),
    renderProcesses(pane),
  );
}

function renderPaneState(pane) {
  const program = pane.foreground_program ?? pane.current_command;
  switch (pane.state) {
    case "at_prompt":
      return pill(`${program} · idle`, "pill-good", "A shell is waiting for a command");
    case "running":
      return pill(program, "pill-busy", `${program} is running in the foreground`);
    case "exited":
      return pill("exited", "", "The pane's program exited");
    default:
      return pill("unknown", "", "No foreground process found");
  }
}

function renderClaudeCodePill(session) {
  const status = session.status ?? "unknown";
  return pill(`claude ${status}`, CLAUDE_CODE_PILLS[status] ?? "", `Claude Code reports “${status}”`);
}

function renderProcesses(pane) {
  const processes = pane.processes;
  const rows = processes.map((process) =>
    make(
      "tr",
      { title: process.command },
      make("td", { text: process.pid }),
      make("td", { text: process.name }),
      make("td", { text: process.state }),
      make("td", { text: process.in_foreground ? "foreground" : "" }),
      make("td", { text: `${process.cpu_percent.toFixed(1)}% CPU` }),
    ),
  );
  const noun = processes.length === 1 ? "process" : "processes";
  const details = make(
    "details",
    { className: "processes" },
    make("summary", { text: `${processes.length} ${noun}` }),
    make("table", {}, ...rows),
  );
  details.dataset.pane = pane.id;
  return details;
}

// Like a shell prompt: branch ✔ staged ✘ unstaged ✂ untracked.
function renderGitStatus(git) {
  const branch = git.branch ?? `@${git.commit ?? "?"}`;
  const branchTitle = git.branch
    ? `Branch ${git.branch}${git.upstream ? `, tracking ${git.upstream}` : ""}`
    : "Detached HEAD";
  const parts = [
    make("span", { className: `git-branch${git.dirty ? " dirty" : ""}`, text: branch, title: branchTitle }),
  ];
  const counts = [
    [git.staged, "git-staged", "✔", "staged"],
    [git.unstaged, "git-unstaged", "✘", "unstaged"],
    [git.untracked, "git-untracked", "✂", "untracked"],
    [git.conflicted, "git-conflicted", "⚠", "conflicted"],
  ];
  for (const [count, className, symbol, label] of counts) {
    if (count > 0) parts.push(make("span", { className, text: `${symbol}${count}`, title: `${count} ${label}` }));
  }
  if (git.ahead > 0 || git.behind > 0) {
    const sync = [git.ahead > 0 ? `↑${git.ahead}` : "", git.behind > 0 ? `↓${git.behind}` : ""].join("");
    const title = `${git.ahead} ahead of, ${git.behind} behind ${git.upstream}`;
    parts.push(make("span", { className: "git-sync", text: sync, title }));
  }
  return make("div", { className: "git" }, ...parts);
}

// Claude Code sessions

function renderClaudeCodeSession(session) {
  let location = "not in tmux";
  if (session.tmux_location) {
    location = session.tmux_pane_found
      ? `tmux ${session.tmux_location}`
      : `tmux ${session.tmux_location} (another tmux server)`;
  }

  return make(
    "li",
    {},
    make("strong", { text: session.name ?? `pid ${session.pid}`, title: `Session ${session.session_id ?? "?"}` }),
    renderClaudeCodePill(session),
    make(
      "div",
      { className: "details" },
      make("div", { className: "muted", text: `${location} · v${session.version ?? "?"} · pid ${session.pid}` }),
      path(session.cwd ?? ""),
    ),
    make("span", { className: "updated", text: timeAgo(session.status_updated_at), title: "Status last changed" }),
  );
}

// Refreshing and actions

let refreshing = false;
let lastRefreshError;

async function refreshStatus() {
  if (refreshing) return;
  refreshing = true;
  try {
    const status = await invoke("get_tmux_status", { tmuxPath: readSettings().tmux_path });

    const sessions = el("sessions");
    if (status.sessions.length === 0) {
      sessions.replaceChildren(make("div", { className: "card empty muted", text: "No tmux sessions." }));
    } else {
      // Keep open process lists open across refreshes.
      const openProcessLists = new Set(
        [...sessions.querySelectorAll("details[open]")].map((details) => details.dataset.pane),
      );
      sessions.replaceChildren(...status.sessions.map(renderSession));
      for (const details of sessions.querySelectorAll("details.processes")) {
        if (openProcessLists.has(details.dataset.pane)) details.open = true;
      }
    }

    const claudeCodeSessions = status.claude_code_sessions;
    el("claude-code-sessions").replaceChildren(
      ...(claudeCodeSessions.length > 0
        ? claudeCodeSessions.map(renderClaudeCodeSession)
        : [make("li", { className: "empty muted", text: "No Claude Code sessions running." })]),
    );
    lastRefreshError = undefined;
  } catch (error) {
    // Refreshes repeat, so only log an error once until it changes.
    if (String(error) !== lastRefreshError) log(`Getting the tmux status failed: ${error}`);
    lastRefreshError = String(error);
  } finally {
    refreshing = false;
  }
}

async function openSession(name) {
  try {
    const outcome = await invoke("open_tmux_session", { settings: readSettings(), name });
    log(`${name}: ${OUTCOMES[outcome] ?? outcome}`);
  } catch (error) {
    log(`Opening ${name} failed: ${error}`);
  }
  await refreshStatus();
}

async function createSession(event) {
  event.preventDefault();
  const form = new FormData(event.target);
  const name = form.get("name").trim();
  const directory = form.get("directory");
  const command = form.get("command");
  try {
    const outcome = await invoke("create_tmux_session", {
      settings: readSettings(),
      name,
      directory,
      command,
    });
    log(`Created ${name}: ${OUTCOMES[outcome] ?? outcome}`);
  } catch (error) {
    log(`Creating ${name} failed: ${error}`);
  }
  await refreshStatus();
}

// The clipboard API can be unavailable in the webview, so fall back to
// selecting the text for the user to copy.
async function copyText(text, element) {
  try {
    await navigator.clipboard.writeText(text);
    log(`Copied: ${text}`);
  } catch {
    window.getSelection().selectAllChildren(element);
    log("Could not copy, the command is selected instead");
  }
}

window.addEventListener("DOMContentLoaded", async () => {
  el("new-session").addEventListener("submit", createSession);
  el("refresh").addEventListener("click", refreshStatus);
  el("settings").addEventListener("change", renderSettingsSummary);
  el("settings").elements.tmux_path.addEventListener("change", refreshStatus);
  window.addEventListener("focus", refreshStatus);

  try {
    renderSettings(await invoke("get_tmux_example_settings"));
  } catch (error) {
    el("settings-summary").textContent = `Detecting failed: ${error}`;
  }
  await refreshStatus();
  setInterval(refreshStatus, REFRESH_INTERVAL_MS);
});
