<!-- The quick search popup. The Rust side (src-tauri/src/quick_search.rs)
     opens it with ⌃⌘T, knows the choices (text snippets and sample images)
     and does the copying and pasting; this page filters the choices as the
     user types, moves the selection with the arrow keys and sends the pick. -->
<script>
  import { onMount, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";

  const ANCHORS = {
    caret: "At the text caret",
    text_field: "Under the text field",
    screen: "No caret found",
  };
  const APP_SUFFIX = (opened) => (opened?.previous_app?.name ? ` in ${opened.previous_app.name}` : "");

  let choices = $state([]);
  let query = $state("");
  let selected = $state(0);
  let opened = $state();
  let message = $state("");
  let input = $state();

  let matches = $derived.by(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) return choices;
    return choices.filter(
      (choice) => choice.label.toLowerCase().includes(needle) || choice.value.toLowerCase().includes(needle),
    );
  });

  // A new filter starts over at the first match.
  $effect(() => {
    query;
    selected = 0;
  });

  let anchorMessage = $derived.by(() => {
    if (!opened) return "";
    const detail = opened.anchor_detail ? `: ${opened.anchor_detail}` : "";
    return `${ANCHORS[opened.anchor] ?? opened.anchor}${APP_SUFFIX(opened)}${detail}`;
  });

  // Each time the popup opens it starts over with the input focused.
  async function reset(event) {
    query = "";
    message = "";
    opened = event?.payload;
    await tick();
    input?.focus();
  }

  async function pick(choice) {
    if (!choice) return;
    message = "Copying…";
    try {
      await invoke("quick_search_pick", { choiceId: choice.id, paste: true });
    } catch (error) {
      message = `Failed: ${error}`;
    }
  }

  function onSubmit(event) {
    event.preventDefault();
    pick(matches[selected] ?? matches[0]);
  }

  function onKeydown(event) {
    if (event.key === "Escape") {
      event.preventDefault();
      invoke("quick_search_dismiss");
    } else if (event.key === "ArrowDown" && matches.length) {
      event.preventDefault();
      selected = (selected + 1) % matches.length;
    } else if (event.key === "ArrowUp" && matches.length) {
      event.preventDefault();
      selected = (selected - 1 + matches.length) % matches.length;
    }
  }

  onMount(() => {
    invoke("list_quick_search_choices").then((list) => (choices = list));
    const unlisteners = [
      listen("quick-search-opened", reset),
      // The caret found after the popup already opened at the screen position.
      listen("quick-search-anchored", (event) => (opened = event.payload)),
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
  <ul class="choices" role="listbox">
    {#each matches as choice, index (choice.id)}
      <li role="option" aria-selected={index === selected}>
        <button type="button" class="choice" class:selected={index === selected} onclick={() => pick(choice)}>
          {#if choice.preview}
            <img class="thumbnail" src={choice.preview} alt="" />
          {:else}
            <span class="thumbnail text-mark" aria-hidden="true">Aa</span>
          {/if}
          <span class="label">{choice.label}</span>
          <span class="value">{choice.value}</span>
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
    <p class="status" title={anchorMessage}>{message || anchorMessage}</p>
    <p class="keys"><kbd>↑</kbd><kbd>↓</kbd> select · <kbd>↵</kbd> copy &amp; paste · <kbd>esc</kbd> close</p>
  </footer>
</form>
