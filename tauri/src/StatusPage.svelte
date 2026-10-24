<!-- The status window. It mirrors the connection state the Rust side emits
     and sends the user's actions back as commands. -->
<script>
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import Button from "$phoenix/components/Button.svelte";
  import Header from "$phoenix/components/Header.svelte";
  import ThemeToggle from "$phoenix/components/ThemeToggle.svelte";
  import { setAppEmitContext } from "./lib/app-emit.js";
  import { log } from "./lib/log.svelte.js";
  import LogList from "./lib/LogList.svelte";

  setAppEmitContext();

  const LABELS = {
    disconnected: "Disconnected",
    connecting: "Connecting…",
    connected: "Connected, joining the api channel…",
    joined: "Connected",
  };

  let status = $state();
  let quickSearch = $state();

  const PASTE_RESULTS = {
    pasted: "pasted into",
    no_accessibility_access: "not pasted: no Accessibility access",
    previous_app_not_activated: "not pasted: couldn't bring back",
    failed: "not pasted: failed in",
  };

  function render(next) {
    if (next.state !== status?.state) {
      log(`${LABELS[next.state] ?? next.state}${next.detail ? ` — ${next.detail}` : ""}`);
    }
    status = next;
  }

  async function ping() {
    const started = performance.now();
    try {
      const reply = await invoke("api_request", { event: "auth.token.get", payload: {} });
      log(`auth.token.get replied in ${Math.round(performance.now() - started)}ms`, reply);
    } catch (error) {
      log("auth.token.get failed", error);
    }
  }

  async function refreshQuickSearch() {
    try {
      quickSearch = await invoke("get_quick_search_status");
    } catch (error) {
      log(`Getting the quick search status failed: ${error}`);
    }
  }

  async function requestAccessibility() {
    const trusted = await invoke("request_accessibility_access");
    log(trusted ? "Accessibility access is granted" : "Asked macOS for Accessibility access, grant it in System Settings");
    await refreshQuickSearch();
  }

  const PLACEMENTS = { caret: "at the text caret", text_field: "under the text field", screen: "at the screen position" };

  function logOpened(verb, opened) {
    const app = opened.previous_app?.name ?? "no app";
    const [x, y] = opened.position.map(Math.round);
    const error = opened.placement_error ? ` (${opened.placement_error})` : "";
    log(`Quick search ${verb} ${PLACEMENTS[opened.placement] ?? opened.placement} over ${app}, at ${x},${y}${error}`);
  }

  function logPasted(outcome) {
    const app = outcome.previous_app?.name ?? "the previous app";
    const detail = outcome.paste.detail ? ` (${outcome.paste.detail})` : "";
    log(
      `Quick search copied ${outcome.label} as ${outcome.clipboard_types.join(", ")}, ${PASTE_RESULTS[outcome.paste.result]} ${app}${detail}`,
    );
  }

  async function run(action, command, args) {
    try {
      await invoke(command, args);
    } catch (error) {
      log(`${action} failed: ${error}`);
    }
  }

  onMount(() => {
    const unlisteners = [
      listen("connection-status", (event) => render(event.payload)),
      listen("api-message", (event) => log(`Server pushed ${event.payload.event}`, event.payload.payload)),
      listen("quick-search-opened", (event) => logOpened("opened", event.payload)),
      listen("quick-search-moved", (event) => logOpened("moved", event.payload)),
      listen("quick-search-pasted", (event) => logPasted(event.payload)),
    ];
    invoke("get_connection_status").then(render);
    // The user grants Accessibility access in System Settings and comes back.
    refreshQuickSearch();
    window.addEventListener("focus", refreshQuickSearch);
    return () => {
      window.removeEventListener("focus", refreshQuickSearch);
      unlisteners.forEach(async (unlisten) => (await unlisten)());
    };
  });
</script>

<main class="page">
  <Header>
    Starter Pack
    {#snippet subtitle()}
      Server: <code>{status?.server_url ?? "…"}</code>
    {/snippet}
    {#snippet actions()}
      <ThemeToggle />
    {/snippet}
  </Header>

  <section class="connection" data-state={status?.state ?? "disconnected"}>
    <span class="dot" aria-hidden="true"></span>
    <div>
      <p class="connection-label">{status ? (LABELS[status.state] ?? status.state) : "Starting…"}</p>
      <p class="muted">{status?.detail ?? ""}</p>
    </div>
  </section>

  <section class="actions">
    {#if status && !status.has_token}
      <Button variant="primary" onclick={() => run("Log in", "open_phoenix", { path: "/client/new" })}>Log in</Button>
    {/if}
    <Button variant="primary" onclick={() => run("Open Phoenix", "open_phoenix", { path: "/" })}>Open Phoenix</Button>
    {#if status?.state === "joined"}
      <Button variant="primary" onclick={ping}>Send auth.token.get</Button>
    {/if}
    {#if status?.has_token}
      <Button onclick={() => run("Log out", "log_out")}>Log out</Button>
    {/if}
  </section>

  <section>
    <h2>Examples</h2>
    <p><a href="tmux-example.html">tmux sessions in your terminal app →</a></p>
  </section>

  <section>
    <h2>Quick search</h2>
    <p>
      Press <kbd>{quickSearch?.shortcut ?? "⌃⌘T"}</kbd> in any app to open the popup, type to filter, <kbd>↵</kbd> copies
      the selected item and pastes it where you were typing.
    </p>
    <p class="muted">
      {#if quickSearch?.accessibility_trusted}
        Accessibility access is granted, so pasting and finding the text caret work.
      {:else}
        Without Accessibility access the popup can only copy, and opens near the top of the screen. Under
        <code>cargo tauri dev</code> macOS checks the terminal app running it, so grant access to that app when
        the system dialog names it.
      {/if}
    </p>
    <div class="actions">
      <Button onclick={() => run("Open quick search", "open_quick_search")}>Open quick search</Button>
      {#if quickSearch && !quickSearch.accessibility_trusted}
        <Button variant="primary" onclick={requestAccessibility}>Allow Accessibility access</Button>
      {/if}
    </div>
  </section>

  <LogList />
</main>
