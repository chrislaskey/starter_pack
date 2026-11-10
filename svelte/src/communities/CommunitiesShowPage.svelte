<!-- One community, at /communities/:id, to join or leave, and for a
     member to edit or delete -->
<script>
  import AppNav from "../lib/AppNav.svelte";
  import { communitiesDelete, communitiesGet, onSynced } from "../lib/core/index.js";
  import { navigateToUrl, pathParams } from "../lib/router.js";
  import { communitiesEditUrl, communitiesIndexUrl } from "../lib/routes.js";
  import CommunitiesDeleteDialog from "./CommunitiesDeleteDialog.svelte";
  import CommunitiesMembershipButton from "./CommunitiesMembershipButton.svelte";

  const id = pathParams().id;
  let shown = $state();
  let error = $state("");
  let deleting = $state(false);
  let busy = $state(false);

  async function load() {
    try {
      shown = await communitiesGet(id);
      error = "";
    } catch (problem) {
      error = String(problem);
    }
  }

  async function remove() {
    busy = true;
    try {
      await communitiesDelete(id);
      deleting = false;
      await navigateToUrl(communitiesIndexUrl());
    } catch (problem) {
      deleting = false;
      error = String(problem);
    } finally {
      busy = false;
    }
  }

  $effect(() => onSynced(load));
  load();
</script>

<main class="page">
  <AppNav />
  <h1>{shown ? shown.community.name : "Community"}</h1>

  {#if error}
    <p class="error">{error}</p>
  {/if}
  {#if shown}
    <p class="muted">
      Created {shown.community.inserted_at}
      {#if shown.membership}
        · you are a member
      {/if}
    </p>
    <div class="flex flex-wrap gap-2">
      <CommunitiesMembershipButton
        communityId={id}
        membership={shown.membership}
        onchange={load}
        onerror={(problem) => (error = problem)}
      />
      {#if shown.membership}
        <a href={communitiesEditUrl(id)} class="btn btn-sm">Edit</a>
        <button type="button" class="btn btn-sm btn-ghost" onclick={() => (deleting = true)}>Delete</button>
      {/if}
    </div>
    <CommunitiesDeleteDialog
      community={deleting ? shown.community : null}
      {busy}
      ondelete={remove}
      oncancel={() => (deleting = false)}
    />
  {/if}

  <a href={communitiesIndexUrl()}>Back to communities</a>
</main>
