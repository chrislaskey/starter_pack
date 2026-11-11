<!-- Asked after a sign-in on a device that has a local user: whether
     what they made moves into the account that just signed in, or the
     two stay apart, see docs/design/accounts-local-and-trial-users.md. -->
<script>
  /**
   * @typedef {Object} Props
   * @property {{email: string, items: number} | null} offered the move offered; the dialog is open while set
   * @property {string} [error]
   * @property {boolean} [busy]
   * @property {() => void} onmove
   * @property {() => void} onkeepseparate
   */

  /** @type {Props} */
  let { offered, error = "", busy = false, onmove, onkeepseparate } = $props();

  /** @type {HTMLDialogElement | undefined} */
  let dialog = $state();

  $effect(() => {
    if (!dialog) return;
    if (offered && !dialog.open) dialog.showModal();
    if (!offered && dialog.open) dialog.close();
  });
</script>

<dialog bind:this={dialog} class="modal" oncancel={(event) => event.preventDefault()}>
  <div class="modal-box flex flex-col gap-3">
    {#if offered}
      <h2>Move {offered.items} {offered.items === 1 ? "item" : "items"} into {offered.email}?</h2>
      <p>
        What you made on this device becomes part of <strong>{offered.email}</strong> and syncs with it. The account
        that was only on this device is gone afterwards.
      </p>
      <p class="muted text-sm">
        Keeping them separate adds {offered.email} beside it, and leaves what you made where it is, on this device
        only.
      </p>
      {#if error}
        <p class="error">{error}</p>
      {/if}
      <div class="flex flex-wrap justify-end gap-2">
        <button type="button" class="btn btn-sm btn-ghost" onclick={onkeepseparate} disabled={busy}>Keep them separate</button>
        <button type="button" class="btn btn-sm btn-primary" onclick={onmove} disabled={busy} data-move>Move</button>
      </div>
    {/if}
  </div>
</dialog>
