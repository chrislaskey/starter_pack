<!-- The communities the user is a member of, on this device, to open or
     leave, as App.Web.CommunitiesLive lists them. Creating and joining a
     community need the server, so they are on the web for now, see
     docs/suggestions/offline-pages.md. -->
<script>
  import { invoke } from "@tauri-apps/api/core";
  import Button from "$phoenix/components/Button.svelte";
  import Header from "$phoenix/components/Header.svelte";
  import ThemeToggle from "$phoenix/components/ThemeToggle.svelte";
  import AppNav from "../lib/AppNav.svelte";
  import SyncBar from "../lib/SyncBar.svelte";
  import { communityShowUrl } from "../lib/routes.js";
  import { write } from "../lib/write.js";

  let page = $state();
  let error = $state("");

  async function load() {
    try {
      page = await invoke("get_communities_page");
    } catch (problem) {
      error = String(problem);
    }
  }

  async function leave(community) {
    if (!confirm(`Leave ${community.name}?`)) return;
    error = await write("leave_community", { communityId: community.id }, load);
  }
</script>

<main class="page">
  <AppNav />
  <Header>
    Communities
    {#snippet subtitle()}
      Yours, to open and leave
    {/snippet}
    {#snippet actions()}
      <ThemeToggle />
    {/snippet}
  </Header>

  <SyncBar onrefresh={load} />

  {#if error}
    <p class="error">{error}</p>
  {/if}

  {#if page}
    {#if page.communities.length === 0}
      <p class="muted">Join a community in the web UI; it appears here on the next sync.</p>
    {/if}
    <ul id="communities" class="divide-y divide-base-200">
      {#each page.communities as community (community.id)}
        <li id="communities-{community.id}" class="flex items-center justify-between py-3">
          <a href={communityShowUrl(community.id)} class="font-medium hover:underline">{community.name}</a>
          <Button type="button" onclick={() => leave(community)}>Leave</Button>
        </li>
      {/each}
    </ul>
  {/if}
</main>
