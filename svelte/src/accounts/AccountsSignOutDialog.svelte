<!-- Asked before a sign-out that would lose changes: how many have not
     reached the server or were not kept by it, with a link to view them,
     and the choice to sign out anyway, losing them. -->
<script>
  import { conflictsIndexUrl } from "../lib/routes.js";

  /**
   * @typedef {Object} Props
   * @property {{pending: number, conflicts: number} | null} loss what would be lost; the dialog is open while set
   * @property {boolean} [busy]
   * @property {() => void} onsignout
   * @property {() => void} oncancel
   */

  /** @type {Props} */
  let { loss, busy = false, onsignout, oncancel } = $props();

  /** @type {HTMLDialogElement | undefined} */
  let dialog = $state();

  $effect(() => {
    if (!dialog) return;
    if (loss && !dialog.open) dialog.showModal();
    if (!loss && dialog.open) dialog.close();
  });

  /** @param {number} count @param {string} one @param {string} many */
  const counted = (count, one, many) => `${count} ${count === 1 ? one : many}`;
</script>

<dialog bind:this={dialog} class="modal" onclose={oncancel}>
  <div class="modal-box flex flex-col gap-3">
    <h2>Some changes would be lost</h2>
    {#if loss}
      <ul class="list-disc pl-5">
        {#if loss.pending > 0}
          <li>{counted(loss.pending, "change has", "changes have")} not reached the server yet.</li>
        {/if}
        {#if loss.conflicts > 0}
          <li>{counted(loss.conflicts, "change was", "changes were")} not kept by the server.</li>
        {/if}
      </ul>
    {/if}
    <p>Signing out drops them from this device. You can view them first and sync, reapply or let each go.</p>
    <div class="flex flex-wrap justify-end gap-2">
      <a href={conflictsIndexUrl()} class="btn btn-sm" onclick={oncancel}>View changes</a>
      <button type="button" class="btn btn-sm btn-ghost" onclick={oncancel} disabled={busy}>Stay signed in</button>
      <button type="button" class="btn btn-sm btn-error" onclick={onsignout} disabled={busy}>Sign out anyway</button>
    </div>
  </div>
</dialog>
