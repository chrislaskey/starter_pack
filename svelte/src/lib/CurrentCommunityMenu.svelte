<!-- The current community in the nav, by name, as a button that opens a
     dialog in the middle of the screen: the person's communities, a
     filter above them, and a link to the communities page. The filter
     takes the focus on a screen with a mouse, not on a touch screen,
     where the keyboard would cover the list. Enter in the filter
     chooses the first match. Choosing one reloads the page in it. -->
<script>
  import Icon from "./Icon.svelte";
  import { communitiesList, currentCommunityUpdate, onSynced } from "./core/index.js";
  import { currentCommunityLoad, onCurrentCommunityChanged } from "./current_community.js";
  import { communitiesIndexUrl } from "./routes.js";

  /** @type {{id: string, name: string} | null} */
  let currentCommunity = $state(null);
  /** @type {{id: string, name: string}[]} */
  let mine = $state([]);
  let filter = $state("");
  let error = $state("");
  let busy = $state(false);
  /** @type {HTMLDialogElement | undefined} */
  let dialog = $state();
  /** @type {HTMLInputElement | undefined} */
  let filterInput = $state();

  let shown = $derived(
    mine.filter((community) => community.name.toLowerCase().includes(filter.trim().toLowerCase())),
  );

  async function load() {
    try {
      currentCommunity = await currentCommunityLoad();
    } catch {
      currentCommunity = null;
    }
  }

  async function open() {
    filter = "";
    error = "";
    dialog?.showModal();
    if (!matchMedia("(pointer: coarse)").matches) filterInput?.focus();
    try {
      mine = (await communitiesList()).filter(({ membership }) => membership !== null).map(({ community }) => community);
    } catch (problem) {
      error = String(problem);
    }
  }

  /** @param {string} communityId */
  async function choose(communityId) {
    busy = true;
    try {
      await currentCommunityUpdate(communityId);
      window.location.reload();
    } catch (problem) {
      error = String(problem);
      busy = false;
    }
  }

  /** @param {KeyboardEvent} event */
  function chooseFirstOnEnter(event) {
    if (event.key !== "Enter" || event.isComposing) return;
    event.preventDefault();
    if (shown.length > 0 && !busy) choose(shown[0].id);
  }

  $effect(() => onSynced(load));
  $effect(() => onCurrentCommunityChanged(load));
  load();
</script>

<button type="button" class="btn btn-ghost btn-sm gap-1" onclick={open}>
  <Icon name="hero-user-group-mini" class="size-5" />
  {currentCommunity ? currentCommunity.name : "No community"}
  <Icon name="hero-chevron-down-mini" class="size-4" />
</button>

<dialog bind:this={dialog} class="modal">
  <div class="modal-box flex max-h-[80vh] flex-col gap-3">
    <div class="flex items-center justify-between gap-2">
      <h2>Communities</h2>
      <button type="button" class="btn btn-ghost btn-sm btn-square" aria-label="Close" onclick={() => dialog?.close()}>
        <Icon name="hero-x-mark-mini" class="size-5" />
      </button>
    </div>
    <input
      bind:this={filterInput}
      bind:value={filter}
      onkeydown={chooseFirstOnEnter}
      type="search"
      class="input w-full"
      placeholder="Filter communities"
      aria-label="Filter communities"
    />
    {#if error}
      <p class="error">{error}</p>
    {/if}
    <ul class="menu w-full flex-nowrap overflow-y-auto p-0">
      {#each shown as community (community.id)}
        <li>
          <button
            type="button"
            class={community.id === currentCommunity?.id ? "menu-active" : ""}
            onclick={() => choose(community.id)}
            disabled={busy}
          >
            {community.name}
            {#if community.id === currentCommunity?.id}
              <Icon name="hero-check-mini" class="size-4" />
            {/if}
          </button>
        </li>
      {:else}
        <li class="muted px-3 py-2">
          {mine.length === 0 ? "You are not in a community yet." : "No community matches."}
        </li>
      {/each}
    </ul>
    <a href={communitiesIndexUrl()} class="link" onclick={() => dialog?.close()}>Join / Manage Communities</a>
  </div>
  <form method="dialog" class="modal-backdrop">
    <button type="submit">Close</button>
  </form>
</dialog>
