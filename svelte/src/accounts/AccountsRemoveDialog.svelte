<!-- Asked before an account is removed from this device: what removal
     means, and, when changes would be lost, how many and the choice to
     remove anyway. -->
<script>
  import { conflictsIndexUrl } from "../lib/routes.js";

  /**
   * @typedef {Object} Props
   * @property {{email: string, active: boolean} | null} account the account to remove; the dialog is open while set
   * @property {{pending: number, conflicts: number} | null} loss what a removal would lose, once the first attempt said so
   * @property {boolean} [busy]
   * @property {(losingChanges: boolean) => void} onremove
   * @property {() => void} oncancel
   */

  /** @type {Props} */
  let { account, loss, busy = false, onremove, oncancel } = $props();

  /** @type {HTMLDialogElement | undefined} */
  let dialog = $state();

  $effect(() => {
    if (!dialog) return;
    if (account && !dialog.open) dialog.showModal();
    if (!account && dialog.open) dialog.close();
  });

  /** @param {number} count @param {string} one @param {string} many */
  const counted = (count, one, many) => `${count} ${count === 1 ? one : many}`;
</script>

<dialog bind:this={dialog} class="modal" onclose={oncancel}>
  <div class="modal-box flex flex-col gap-3">
    {#if account}
      <h2>Remove {account.email} from this device?</h2>
      {#if loss}
        <ul class="list-disc pl-5">
          {#if loss.pending > 0}
            <li>{counted(loss.pending, "change has", "changes have")} not reached the server yet.</li>
          {/if}
          {#if loss.conflicts > 0}
            <li>{counted(loss.conflicts, "change was", "changes were")} not kept by the server.</li>
          {/if}
        </ul>
        <p>Removing the account drops them from this device.</p>
      {:else}
        <p>
          Any data stored on this device for this account is removed. What was synced stays on the server and on other
          devices, and signing in again later downloads it to this device again.
        </p>
      {/if}
      <div class="flex flex-wrap justify-end gap-2">
        {#if loss && account.active}
          <a href={conflictsIndexUrl()} class="btn btn-sm" onclick={oncancel}>View changes</a>
        {/if}
        <button type="button" class="btn btn-sm btn-ghost" onclick={oncancel} disabled={busy}>Keep</button>
        <button type="button" class="btn btn-sm btn-error" onclick={() => onremove(loss !== null)} disabled={busy}>
          {loss ? "Remove anyway" : "Remove from device"}
        </button>
      </div>
    {/if}
  </div>
</dialog>
