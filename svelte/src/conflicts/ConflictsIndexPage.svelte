<!-- The changes made here that are not on the server, at /conflicts:
     those waiting to reach it, which the next sync sends, and those it
     did not keep, each to reapply, or to try again when the server
     refused it outright, or let go -->
<script>
  import AppNav from "../lib/AppNav.svelte";
  import {
    conflictsLetGo,
    conflictsList,
    conflictsReapply,
    conflictsTryAgain,
    onSynced,
    pendingChangesList,
  } from "../lib/core/index.js";
  import { postsIndexUrl } from "../lib/routes.js";

  let pending = $state([]);
  let conflicts = $state([]);
  let error = $state("");

  async function load() {
    try {
      [pending, conflicts] = await Promise.all([pendingChangesList(), conflictsList()]);
    } catch (problem) {
      error = String(problem);
    }
  }

  /** @param {() => Promise<unknown>} resolve */
  async function resolveThen(resolve) {
    error = "";
    try {
      await resolve();
      await load();
    } catch (problem) {
      error = String(problem);
    }
  }

  /** @param {unknown} value */
  const shown = (value) => (value === null || value === undefined ? "" : String(value));

  $effect(() => onSynced(load));
  load();
</script>

<main class="page">
  <AppNav />
  <h1>Changes not on the server</h1>

  {#if error}
    <p class="error">{error}</p>
  {/if}

  {#if pending.length === 0 && conflicts.length === 0 && !error}
    <p class="muted">Every change reached the server.</p>
  {/if}

  {#if pending.length > 0}
    <h2>Waiting to reach the server</h2>
    <ul class="posts">
      {#each pending as change (change.id)}
        <li class="post">
          <p class="muted">{change.about ?? change.table} · {change.kind} · {change.made_at}</p>
          {#each Object.entries(change.changes) as [name, value] (name)}
            <p><strong>{name}</strong></p>
            <p class="whitespace-pre-wrap">{shown(value)}</p>
          {/each}
        </li>
      {/each}
    </ul>
  {/if}

  {#if conflicts.length > 0}
    <h2>Not kept by the server</h2>
  {/if}
  <ul class="posts">
    {#each conflicts as conflict (conflict.id)}
      <li class="post">
        <p class="muted">{conflict.about ?? "Something that is gone"} · {conflict.made_at}</p>
        {#if conflict.reason}
          <p class="error text-sm">{conflict.reason}</p>
        {/if}
        {#if conflict.resolve === "try_again"}
          <p>The server did not take this. It is still on this device.</p>
        {:else}
          {#each conflict.fields as field (field.name)}
            <p><strong>Your change</strong></p>
            <p class="whitespace-pre-wrap">{shown(field.yours)}</p>
            <p><strong>Now on the server</strong></p>
            <p class="whitespace-pre-wrap">{shown(field.now)}</p>
          {/each}
        {/if}
        <div class="flex gap-2">
          {#if conflict.resolve === "reapply"}
            <button type="button" class="btn btn-sm" onclick={() => resolveThen(() => conflictsReapply(conflict.id))}>
              Reapply
            </button>
          {:else if conflict.resolve === "try_again"}
            <button type="button" class="btn btn-sm" onclick={() => resolveThen(() => conflictsTryAgain(conflict.id))}>
              Try again
            </button>
          {/if}
          <button type="button" class="btn btn-sm btn-ghost" onclick={() => resolveThen(() => conflictsLetGo(conflict.id))}>
            Let it go
          </button>
        </div>
      </li>
    {/each}
  </ul>

  <a href={postsIndexUrl()}>Back to posts</a>
</main>
