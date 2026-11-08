<!-- Shown while a sign-in waits on the browser, inside the app: the
     browser has the web's sign-in page and ends by showing a code, which
     the person pastes here, see docs/design/accounts-sign-in.md. -->
<script>
  /**
   * @typedef {Object} Props
   * @property {{email: string | null} | null} pending the sign-in waiting; the dialog is open while set
   * @property {string} [error]
   * @property {boolean} [busy]
   * @property {(code: string) => void} onfinish
   * @property {() => void} onopenbrowser the browser again, with a fresh start
   * @property {() => void} oncancel
   */

  /** @type {Props} */
  let { pending, error = "", busy = false, onfinish, onopenbrowser, oncancel } = $props();

  /** @type {HTMLDialogElement | undefined} */
  let dialog = $state();
  let code = $state("");

  $effect(() => {
    if (!dialog) return;
    if (pending && !dialog.open) {
      code = "";
      dialog.showModal();
    }
    if (!pending && dialog.open) dialog.close();
  });

  function submit(event) {
    event.preventDefault();
    onfinish(code.trim());
  }
</script>

<dialog bind:this={dialog} class="modal" onclose={() => pending && oncancel()}>
  <div class="modal-box flex flex-col gap-3">
    {#if pending}
      <h2>Finish signing in in your browser</h2>
      <p>
        {#if pending.email}
          The sign-in page for <strong>{pending.email}</strong> opened in your browser.
        {:else}
          The sign-in page opened in your browser.
        {/if}
        When it shows a code, paste it here.
      </p>
      <form onsubmit={submit} class="flex flex-col gap-3">
        <input bind:value={code} type="text" autocomplete="off" class="input w-full" aria-label="Code from the browser" placeholder="Code from the browser" required />
        {#if error}
          <p class="error">{error}</p>
        {/if}
        <div class="flex flex-wrap justify-end gap-2">
          <button type="button" class="btn btn-sm btn-ghost" onclick={oncancel} disabled={busy}>Cancel</button>
          <button type="button" class="btn btn-sm" onclick={onopenbrowser} disabled={busy}>Open the browser again</button>
          <button type="submit" class="btn btn-sm btn-primary" disabled={busy}>Finish</button>
        </div>
      </form>
    {/if}
  </div>
</dialog>
