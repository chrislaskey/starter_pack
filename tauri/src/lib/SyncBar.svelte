<!-- Above every page that shows synced data: Sync now, what the last sync
     did, how many changes wait to be pushed, and the changes the server
     kept its own values over, to reapply or let go.

     On mount it learns who is logged in, then calls `onrefresh` for the
     page to read itself, and syncs. It calls `onrefresh` again after every
     sync, whoever started it. -->
<script>
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import Button from "$phoenix/components/Button.svelte";

  /** @type {{ onrefresh: () => Promise<void> }} */
  let { onrefresh } = $props();

  let user = $state();
  let status = $state();
  let syncing = $state(false);
  let lastSync = $state("");
  let error = $state("");
  let showConflicts = $state(false);

  function describe(report) {
    return `Synced: ${report.pushed} pushed, ${report.rejected} rejected, ${report.conflicts} not saved, ${report.pulled} pulled, ${report.backfilled} backfilled, ${report.pruned} pruned`;
  }

  async function refresh() {
    try {
      status = await invoke("get_sync_status");
      error = "";
    } catch (problem) {
      error = String(problem);
    }
    await onrefresh();
  }

  async function sync() {
    syncing = true;
    try {
      lastSync = describe(await invoke("sync_now"));
    } catch (problem) {
      lastSync = `Sync failed, working offline: ${problem}`;
    } finally {
      syncing = false;
    }
  }

  async function resolve(conflict, resolution) {
    try {
      await invoke("resolve_conflict", { eventId: conflict.id, resolution });
      await refresh();
      if (status.conflicts.length === 0) showConflicts = false;
    } catch (problem) {
      error = String(problem);
    }
  }

  function shown(value) {
    if (value === null || value === undefined) return "(none)";
    return typeof value === "string" ? value : JSON.stringify(value);
  }

  onMount(() => {
    const unlisten = listen("synced", (event) => {
      if (event.payload.error) lastSync = `Sync failed, working offline: ${event.payload.error}`;
      refresh();
    });
    (async () => {
      try {
        user = await invoke("get_profile");
      } catch (problem) {
        error = String(problem);
        return;
      }
      await refresh();
      sync();
    })();
    return () => {
      unlisten.then((stop) => stop());
    };
  });
</script>

{#if error}
  <p class="error">{error}</p>
{/if}

{#if user && status}
  <section class="space-y-2">
    <div class="flex flex-wrap items-center gap-3">
      <Button onclick={sync} disabled={syncing}>{syncing ? "Syncing…" : "Sync now"}</Button>
      <span class="muted text-sm">
        {user.email}{lastSync ? ` · ${lastSync}` : ""}{status.pending
          ? ` · ${status.pending} change(s) waiting to sync`
          : ""}
      </span>
    </div>

    {#if status.conflicts.length}
      <div id="conflicts" class="space-y-4 rounded-box border border-warning p-4">
        <p>
          <strong>
            {status.conflicts.length === 1 ? "1 change was" : `${status.conflicts.length} changes were`} not saved on the
            server.
          </strong>
          Someone else changed the same thing first.
          <button type="button" class="link" onclick={() => (showConflicts = !showConflicts)}>
            {showConflicts ? "Hide details" : "See details"}
          </button>
        </p>
        {#if showConflicts}
          <ul class="space-y-4">
            {#each status.conflicts as conflict (conflict.id)}
              <li class="space-y-1">
                <p class="muted">
                  {conflict.summary || "A post"} · changed {conflict.made_at.slice(0, 19).replace("T", " ")}
                </p>
                {#each conflict.fields as field (field.field)}
                  <p>Your change to {field.field}: {shown(field.yours)}</p>
                  <p>Now: {conflict.row_exists ? shown(field.server) : "(it is gone)"}</p>
                {/each}
                <div class="flex gap-2">
                  {#if conflict.row_exists}
                    <Button variant="primary" type="button" onclick={() => resolve(conflict, "reapply")}>Reapply</Button>
                  {/if}
                  <Button type="button" onclick={() => resolve(conflict, "ignore")}>Let it go</Button>
                </div>
              </li>
            {/each}
          </ul>
        {/if}
      </div>
    {/if}
  </section>
{/if}
