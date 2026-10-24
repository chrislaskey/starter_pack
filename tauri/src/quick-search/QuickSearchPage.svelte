<!-- The quick search popup. The Rust side (src-tauri/src/quick_search/)
     opens it with ⌃⌘T, lists and filters the items and pastes the one the
     user picks; this page shows them and moves the selection. -->
<script>
  import { onMount, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";

  const PLACEMENTS = {
    caret: "At the text caret",
    text_field: "Under the text field",
    screen: "No caret found",
  };

  let items = $state([]);
  let query = $state("");
  let selected = $state(0);
  let opened = $state();
  let message = $state("");
  let input = $state();

  let placementMessage = $derived.by(() => {
    if (!opened) return "";
    const app = opened.previous_app?.name ? ` in ${opened.previous_app.name}` : "";
    const error = opened.placement_error ? `: ${opened.placement_error}` : "";
    return `${PLACEMENTS[opened.placement] ?? opened.placement}${app}${error}`;
  });

  // The last query sent wins, in case replies arrive out of order.
  let latestQuery = "";

  async function listItems() {
    const sent = query;
    latestQuery = sent;
    const listed = await invoke("list_quick_search_items", { query: sent });
    if (sent !== latestQuery) return;
    items = listed;
    selected = 0;
  }

  $effect(() => {
    query;
    listItems();
  });

  async function reset(event) {
    query = "";
    message = "";
    opened = event?.payload;
    await tick();
    input?.focus();
  }

  async function paste(item) {
    if (!item) return;
    message = "Copying…";
    try {
      await invoke("paste_quick_search_item", { itemId: item.id });
    } catch (error) {
      message = `Failed: ${error}`;
    }
  }

  function onSubmit(event) {
    event.preventDefault();
    paste(items[selected] ?? items[0]);
  }

  function onKeydown(event) {
    if (event.key === "Escape") {
      event.preventDefault();
      invoke("close_quick_search");
    } else if (event.key === "ArrowDown" && items.length) {
      event.preventDefault();
      selected = (selected + 1) % items.length;
    } else if (event.key === "ArrowUp" && items.length) {
      event.preventDefault();
      selected = (selected - 1 + items.length) % items.length;
    }
  }

  onMount(() => {
    const unlisteners = [
      listen("quick-search-opened", reset),
      listen("quick-search-moved", (event) => (opened = event.payload)),
    ];
    reset();
    return () => unlisteners.forEach(async (unlisten) => (await unlisten)());
  });
</script>

<svelte:window onkeydown={onKeydown} />

<form class="popup" onsubmit={onSubmit}>
  <!-- svelte-ignore a11y_autofocus -->
  <input
    bind:this={input}
    bind:value={query}
    type="text"
    placeholder="Type to filter…"
    autofocus
    autocomplete="off"
    spellcheck="false"
  />
  <ul class="items" role="listbox">
    {#each items as item, index (item.id)}
      <li role="option" aria-selected={index === selected}>
        <button type="button" class="item" class:selected={index === selected} onclick={() => paste(item)}>
          {#if item.preview_url}
            <img class="thumbnail" src={item.preview_url} alt="" />
          {:else}
            <span class="thumbnail text-mark" aria-hidden="true">Aa</span>
          {/if}
          <span class="label">{item.label}</span>
          <span class="value">{item.value}</span>
        </button>
      </li>
    {:else}
      <li class="muted empty">No matches</li>
    {/each}
  </ul>
  <footer>
    {#if opened && !opened.accessibility_trusted}
      <p class="warning">No Accessibility access: copies only, can't paste or find the caret.</p>
    {/if}
    <p class="status" title={placementMessage}>{message || placementMessage}</p>
    <p class="keys"><kbd>↑</kbd><kbd>↓</kbd> select · <kbd>↵</kbd> copy &amp; paste · <kbd>esc</kbd> close</p>
  </footer>
</form>
