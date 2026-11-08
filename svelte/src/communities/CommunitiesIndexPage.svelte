<!-- Every community, at /communities, each to join or leave, and a new one to make -->
<script>
  import AppNav from "../lib/AppNav.svelte";
  import { communitiesCreate, communitiesList, membershipsJoin, membershipsLeave, onSynced } from "../lib/core/index.js";
  import { postsIndexUrl } from "../lib/routes.js";

  let communities = $state([]);
  let name = $state("");
  let error = $state("");
  let busy = $state(false);

  async function load() {
    try {
      communities = await communitiesList();
    } catch (problem) {
      error = String(problem);
    }
  }

  /** @param {() => Promise<unknown>} action */
  async function doThen(action) {
    busy = true;
    error = "";
    try {
      await action();
      await load();
    } catch (problem) {
      error = String(problem);
    } finally {
      busy = false;
    }
  }

  function create(event) {
    event.preventDefault();
    doThen(async () => {
      await communitiesCreate(name);
      name = "";
    });
  }

  $effect(() => onSynced(load));
  load();
</script>

<main class="page">
  <AppNav />
  <h1>Communities</h1>

  <form onsubmit={create} class="flex gap-2">
    <input bind:value={name} class="input" aria-label="Community name" placeholder="A new community" />
    <button type="submit" class="btn btn-primary" disabled={busy}>Create</button>
  </form>

  {#if error}
    <p class="error">{error}</p>
  {/if}

  {#if communities.length === 0 && !error}
    <p class="muted">No communities yet.</p>
  {/if}

  <ul class="posts">
    {#each communities as { community, membership } (community.id)}
      <li class="post flex items-center justify-between gap-4">
        <span>{community.name}</span>
        {#if membership}
          <button type="button" class="btn btn-sm btn-ghost" onclick={() => doThen(() => membershipsLeave(community.id))} disabled={busy}>
            Leave
          </button>
        {:else}
          <button type="button" class="btn btn-sm" onclick={() => doThen(() => membershipsJoin(community.id))} disabled={busy}>
            Join
          </button>
        {/if}
      </li>
    {/each}
  </ul>

  <a href={postsIndexUrl()}>Back to posts</a>
</main>
