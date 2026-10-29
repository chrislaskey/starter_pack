<!-- The tmux example page. The Rust side guesses the settings and does the
     work (see src-tauri/src/tmux_example.rs); this page shows the guesses in a
     form the user can correct and passes them along with every action. -->
<script>
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import Button from "$phoenix/components/Button.svelte";
  import Header from "$phoenix/components/Header.svelte";
  import ThemeToggle from "$phoenix/components/ThemeToggle.svelte";
  import { setAppEmitContext } from "../lib/app-emit.js";
  import { log } from "../lib/log.svelte.js";
  import LogList from "../lib/LogList.svelte";
  import { homeUrl } from "../lib/routes.js";
  import ClaudeCodeSession from "./ClaudeCodeSession.svelte";
  import TmuxSession from "./TmuxSession.svelte";

  setAppEmitContext();

  const REFRESH_INTERVAL_MS = 5000;

  const OUTCOMES = {
    focused: "Brought its window to the front",
    opened: "Opened a new window",
  };

  let settings = $state({ shell: "", terminal_app: "", tmux_path: "" });
  let terminalApps = $state([]);
  let homeDirectory = $state();
  let detectError = $state();
  let newSession = $state({ name: "tauri-example", directory: "", command: "claude" });
  let tmuxStatus = $state();

  let settingsSummary = $derived.by(() => {
    if (detectError) return `Detecting failed: ${detectError}`;
    if (terminalApps.length === 0) return "Detecting…";
    const terminalApp = terminalApps.find((app) => app.id === settings.terminal_app)?.name ?? settings.terminal_app;
    return [settings.shell.trim(), terminalApp, settings.tmux_path.trim()].join(" · ");
  });

  function trimmedSettings() {
    return { ...settings, shell: settings.shell.trim(), tmux_path: settings.tmux_path.trim() };
  }

  let refreshing = false;
  let lastRefreshError;

  async function refreshStatus() {
    if (refreshing) return;
    refreshing = true;
    try {
      tmuxStatus = await invoke("get_tmux_status", { tmuxPath: settings.tmux_path.trim() });
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
      const outcome = await invoke("open_tmux_session", { settings: trimmedSettings(), name });
      log(`${name}: ${OUTCOMES[outcome] ?? outcome}`);
    } catch (error) {
      log(`Opening ${name} failed: ${error}`);
    }
    await refreshStatus();
  }

  async function createSession(event) {
    event.preventDefault();
    const name = newSession.name.trim();
    try {
      const outcome = await invoke("create_tmux_session", {
        settings: trimmedSettings(),
        name,
        directory: newSession.directory,
        command: newSession.command,
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

  onMount(() => {
    let interval;
    invoke("get_tmux_example_settings")
      .then((detected) => {
        settings = detected.settings;
        terminalApps = detected.terminal_apps;
        homeDirectory = detected.home_directory;
        newSession.directory = detected.directory;
      })
      .catch((error) => (detectError = String(error)))
      .then(refreshStatus)
      .then(() => (interval = setInterval(refreshStatus, REFRESH_INTERVAL_MS)));
    return () => clearInterval(interval);
  });
</script>

<svelte:window onfocus={refreshStatus} />

<main class="page wide">
  <p><a href={homeUrl()}>← Back</a></p>
  <Header>
    tmux example
    {#snippet subtitle()}
      Your tmux sessions and what runs in them. Refreshes every 5 seconds.
    {/snippet}
    {#snippet actions()}
      <ThemeToggle />
    {/snippet}
  </Header>

  <section>
    <div class="section-header">
      <h2>Sessions</h2>
      <Button class="btn btn-primary btn-soft btn-sm" onclick={refreshStatus}>Refresh</Button>
    </div>
    <div class="session-list">
      {#if tmuxStatus?.sessions.length === 0}
        <div class="panel empty muted">No tmux sessions.</div>
      {/if}
      {#each tmuxStatus?.sessions ?? [] as session (session.name)}
        <TmuxSession {session} {homeDirectory} onOpen={openSession} onCopy={copyText} />
      {/each}
    </div>
  </section>

  <section>
    <h2>Claude Code</h2>
    <ul class="panel rows">
      {#each tmuxStatus?.claude_code_sessions ?? [] as session (session.pid)}
        <ClaudeCodeSession {session} {homeDirectory} />
      {:else}
        {#if tmuxStatus}
          <li class="empty muted">No Claude Code sessions running.</li>
        {/if}
      {/each}
    </ul>
    <p class="muted hint">
      Read from <code>~/.claude/sessions</code>, which Claude Code doesn't document, so it may change.
    </p>
  </section>

  <section>
    <h2>New session</h2>
    <form class="panel new-session" onsubmit={createSession}>
      <label>
        Name
        <input bind:value={newSession.name} required spellcheck="false" />
      </label>
      <label class="directory-field">
        Directory
        <input bind:value={newSession.directory} required spellcheck="false" placeholder="Detecting…" />
      </label>
      <label>
        Command
        <input bind:value={newSession.command} spellcheck="false" />
      </label>
      <Button variant="primary" type="submit">Create and open</Button>
    </form>
  </section>

  <details class="panel settings">
    <summary>
      <span>Settings</span>
      <span class="muted">{settingsSummary}</span>
    </summary>
    <form class="settings-fields" onsubmit={(event) => event.preventDefault()}>
      <p class="muted">Best guesses, change them if they're wrong.</p>
      <label>
        Shell
        <input bind:value={settings.shell} required spellcheck="false" />
      </label>
      <label>
        Terminal app
        <select bind:value={settings.terminal_app}>
          {#each terminalApps as app (app.id)}
            <option value={app.id} title={app.path ?? ""}>{app.path ? app.name : `${app.name} (not found)`}</option>
          {/each}
        </select>
      </label>
      <label>
        tmux
        <input bind:value={settings.tmux_path} onchange={refreshStatus} required spellcheck="false" />
      </label>
    </form>
  </details>

  <LogList />
</main>
