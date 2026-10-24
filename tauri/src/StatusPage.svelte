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
    ];
    invoke("get_connection_status").then(render);
    return () => unlisteners.forEach(async (unlisten) => (await unlisten)());
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

  <LogList />
</main>
