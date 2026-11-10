<!-- Asked before a community is deleted: what a delete takes with it. -->
<script>
  /**
   * @typedef {Object} Props
   * @property {{name: string} | null} community the community to delete; the dialog is open while set
   * @property {boolean} [busy]
   * @property {() => void} ondelete
   * @property {() => void} oncancel
   */

  /** @type {Props} */
  let { community, busy = false, ondelete, oncancel } = $props();

  /** @type {HTMLDialogElement | undefined} */
  let dialog = $state();

  $effect(() => {
    if (!dialog) return;
    if (community && !dialog.open) dialog.showModal();
    if (!community && dialog.open) dialog.close();
  });
</script>

<dialog bind:this={dialog} class="modal" onclose={oncancel}>
  <div class="modal-box flex flex-col gap-3">
    {#if community}
      <h2>Delete {community.name}?</h2>
      <p>
        Every member leaves it, and the posts in it are taken out of it. The posts themselves stay with the people who
        wrote them.
      </p>
      <div class="flex flex-wrap justify-end gap-2">
        <button type="button" class="btn btn-sm btn-ghost" onclick={oncancel} disabled={busy}>Keep</button>
        <button type="button" class="btn btn-sm btn-error" onclick={ondelete} disabled={busy}>Delete community</button>
      </div>
    {/if}
  </div>
</dialog>
