<!-- Every community, at /communities, each to show, join or leave -->
<script>
  import AppNav from "../lib/AppNav.svelte";
  import { communitiesList, onSynced } from "../lib/core/index.js";
  import { communitiesNewUrl, communitiesShowUrl } from "../lib/routes.js";
  import CommunitiesMembershipButton from "./CommunitiesMembershipButton.svelte";

  let communities = $state([]);
  let error = $state("");

  async function load() {
    try {
      communities = await communitiesList();
      error = "";
    } catch (problem) {
      error = String(problem);
    }
  }

  $effect(() => onSynced(load));
  load();
</script>

<main class="page">
  <AppNav />
  <div class="flex items-center justify-between">
    <h1>Communities</h1>
    <a href={communitiesNewUrl()} class="btn btn-primary btn-sm">New community</a>
  </div>

  {#if error}
    <p class="error">{error}</p>
  {/if}

  {#if communities.length === 0 && !error}
    <p class="muted">No communities yet.</p>
  {/if}

  <ul class="posts">
    {#each communities as { community, membership } (community.id)}
      <li class="post flex items-center justify-between gap-4">
        <span>
          <a href={communitiesShowUrl(community.id)} class="link">{community.name}</a>
          {#if membership}
            <span class="muted">· member</span>
          {/if}
        </span>
        <CommunitiesMembershipButton
          communityId={community.id}
          {membership}
          onchange={load}
          onerror={(problem) => (error = problem)}
        />
      </li>
    {/each}
  </ul>
</main>
