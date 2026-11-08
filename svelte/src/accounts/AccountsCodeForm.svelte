<!-- The second step of a sign-up or a sign-in: the six-digit code that
     was emailed, and the error the attempt came back with. -->
<script>
  /**
   * @typedef {Object} Props
   * @property {string} email where the code went
   * @property {string} code
   * @property {string} [error]
   * @property {boolean} [busy]
   * @property {() => void} onsubmit
   * @property {() => void} onresend a new code to the same email
   * @property {boolean} [resent] a new code was just sent
   * @property {() => void} onstartover back to the email and password
   */

  /** @type {Props} */
  let { email, code = $bindable(""), error = "", busy = false, onsubmit, onresend, resent = false, onstartover } = $props();

  function submit(event) {
    event.preventDefault();
    onsubmit();
  }
</script>

<form onsubmit={submit} class="flex flex-col gap-3">
  <p>We sent a code to <strong>{email}</strong>. Enter it to finish; it expires in ten minutes.</p>
  <input
    bind:value={code}
    type="text"
    inputmode="numeric"
    autocomplete="one-time-code"
    pattern="[0-9 ]*"
    class="input w-full"
    aria-label="Code"
    placeholder="Code"
    required
  />
  {#if error}
    <p class="error">{error}</p>
  {/if}
  {#if resent}
    <p class="muted text-sm">A new code is on its way; the old one no longer works.</p>
  {/if}
  <div class="flex flex-wrap items-center gap-3">
    <button type="submit" class="btn btn-primary" disabled={busy}>Continue</button>
    <button type="button" class="link text-sm" onclick={onresend} disabled={busy}>Send a new code</button>
    <button type="button" class="link text-sm" onclick={onstartover} disabled={busy}>Start over</button>
  </div>
</form>
