<!-- The quick picker's window in the desktop app, opened by ⌃⌘T
     (../../../tauri/src/quick_picker/): the signed-in person's posts
     and images under a filter box. ↑/↓ move the selection; Enter or a
     click copies the item and pastes it where the person was typing;
     Escape closes. With nobody signed in, it says so and opens the app. -->
<script>
  import { tick } from "svelte";
  import FileImage from "../lib/FileImage.svelte";
  import {
    currentUserGet,
    filesContentUrl,
    onQuickPickerOpened,
    quickPickerAccessibilityRequest,
    quickPickerClose,
    quickPickerList,
    quickPickerOpenApp,
    quickPickerPaste,
  } from "../lib/core/index.js";
  import { fileSize } from "../files/file_size.js";

  /** @type {{kind: "text" | "image", id: string, label: string, size: number | null}[]} */
  let items = $state([]);
  let signedIn = $state(true);
  let query = $state("");
  let selected = $state(0);
  let message = $state("");
  let accessibilityTrusted = $state(true);
  /** @type {HTMLInputElement | undefined} */
  let input = $state();
  /** @type {HTMLButtonElement | undefined} */
  let openAppButton = $state();
  /** @type {HTMLUListElement | undefined} */
  let list = $state();

  let matches = $derived.by(() => {
    const wanted = query.trim().toLowerCase();
    return items.filter((item) => item.label.toLowerCase().includes(wanted));
  });

  /** @param {{accessibility_trusted: boolean}} [opened] */
  async function open(opened) {
    query = "";
    selected = 0;
    message = "";
    accessibilityTrusted = opened?.accessibility_trusted ?? true;
    focus();
    let user = null;
    try {
      user = await currentUserGet();
    } catch (problem) {
      message = String(problem);
    }
    if (!user) {
      signedIn = false;
      items = [];
      focus();
      return;
    }
    signedIn = true;
    try {
      items = await quickPickerList();
    } catch (problem) {
      message = String(problem);
    }
    focus();
  }

  async function focus() {
    await tick();
    (signedIn ? input : openAppButton)?.focus();
  }

  /** @param {{kind: string, id: string} | undefined} item */
  async function paste(item) {
    if (!item) return;
    message = "Copying…";
    try {
      await quickPickerPaste(item.kind, item.id);
      message = "";
    } catch (problem) {
      message = String(problem);
    }
  }

  async function openApp() {
    try {
      await quickPickerOpenApp();
    } catch (problem) {
      message = String(problem);
    }
  }

  async function allowAccessibility() {
    accessibilityTrusted = await quickPickerAccessibilityRequest();
  }

  /** @param {SubmitEvent} event */
  function onSubmit(event) {
    event.preventDefault();
    paste(matches[selected] ?? matches[0]);
  }

  /** @param {KeyboardEvent} event */
  function onKeydown(event) {
    if (event.key === "Escape") {
      event.preventDefault();
      quickPickerClose();
    } else if (event.key === "ArrowDown" && matches.length) {
      event.preventDefault();
      select((selected + 1) % matches.length);
    } else if (event.key === "ArrowUp" && matches.length) {
      event.preventDefault();
      select((selected - 1 + matches.length) % matches.length);
    }
  }

  /** @param {number} index */
  async function select(index) {
    selected = index;
    await tick();
    list?.querySelector(".selected")?.scrollIntoView({ block: "nearest" });
  }

  $effect(() => onQuickPickerOpened(open));
  open();
</script>

<svelte:window onkeydown={onKeydown} />

{#if signedIn}
  <form class="quick-picker" onsubmit={onSubmit}>
    <input
      bind:this={input}
      bind:value={query}
      oninput={() => select(0)}
      type="text"
      placeholder="Type to filter…"
      autocomplete="off"
      spellcheck="false"
    />
    <ul class="quick-picker-items" role="listbox" bind:this={list}>
      {#each matches as item, index (`${item.kind}:${item.id}`)}
        <li role="option" aria-selected={index === selected}>
          <button type="button" class="quick-picker-item" class:selected={index === selected} onclick={() => paste(item)}>
            {#if item.kind === "image"}
              <FileImage src={filesContentUrl(item.id)} alt="" class="thumbnail" missing="" />
            {:else}
              <span class="thumbnail text-mark" aria-hidden="true">Aa</span>
            {/if}
            <span class="label">{item.label}</span>
            {#if item.size !== null}
              <span class="size">{fileSize(item.size)}</span>
            {/if}
          </button>
        </li>
      {:else}
        <li class="muted empty">{items.length ? "No matches" : "No posts or images yet"}</li>
      {/each}
    </ul>
    <footer>
      {#if message}
        <p class="error" title={message}>{message}</p>
      {:else if !accessibilityTrusted}
        <p>
          Copies only: <button type="button" class="link" onclick={allowAccessibility}>allow Accessibility access</button> to paste.
        </p>
      {/if}
      <p class="keys"><kbd>↑</kbd><kbd>↓</kbd> select · <kbd>↵</kbd> copy &amp; paste · <kbd>esc</kbd> close</p>
    </footer>
  </form>
{:else}
  <div class="quick-picker">
    <div class="quick-picker-signed-out">
      <p>Sign in to Starter Pack to pick from your posts and images.</p>
      <button type="button" class="btn btn-primary btn-sm" bind:this={openAppButton} onclick={openApp}>Open Starter Pack</button>
      {#if message}
        <p class="error">{message}</p>
      {/if}
    </div>
    <footer>
      <p class="keys"><kbd>↵</kbd> open · <kbd>esc</kbd> close</p>
    </footer>
  </div>
{/if}
