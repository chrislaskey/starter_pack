<!-- The changes made here that the server did not keep, at /conflicts,
     each to reapply or let go -->
<script>
  import AppNav from "../lib/AppNav.svelte";
  import { conflictsLetGo, conflictsList, conflictsReapply, onSynced } from "../lib/core/index.js";
  import { postsIndexUrl } from "../lib/routes.js";

  let conflicts = $state([]);
  let error = $state("");

  async function load() {
    try {
      conflicts = await conflictsList();
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
  <h1>Changes not saved on the server</h1>

  {#if error}
    <p class="error">{error}</p>
  {/if}

  {#if conflicts.length === 0 && !error}
    <p class="muted">Every change reached the server.</p>
  {/if}

  <ul class="posts">
    {#each conflicts as conflict (conflict.id)}
      <li class="post">
        <p class="muted">{conflict.about ?? "A post that is gone"} · {conflict.made_at}</p>
        {#each conflict.fields as field (field.name)}
          <p><strong>Your change</strong></p>
          <p class="whitespace-pre-wrap">{shown(field.yours)}</p>
          <p><strong>Now on the server</strong></p>
          <p class="whitespace-pre-wrap">{shown(field.now)}</p>
        {/each}
        <div class="flex gap-2">
          {#if conflict.about !== null}
            <button type="button" class="btn btn-sm" onclick={() => resolveThen(() => conflictsReapply(conflict.id))}>
              Reapply
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
