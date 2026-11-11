<!-- The step after the code, when there is one: the account's password,
     a new one in place of a forgotten one, or, at a first sign-in, a
     password to make or skip. -->
<script>
  /**
   * @typedef {Object} Props
   * @property {boolean} isNew a password to make, which may be skipped
   * @property {boolean} [resetting] a new password in place of a forgotten one
   * @property {() => void} [onforgot] given when a forgotten password may be reset
   * @property {string} password
   * @property {string} [error]
   * @property {boolean} [busy]
   * @property {() => void} onsubmit
   * @property {() => void} [onskip] for a new password
   * @property {() => void} onstartover back to the email
   */

  /** @type {Props} */
  let { isNew, resetting = false, password = $bindable(""), error = "", busy = false, onsubmit, onskip, onforgot, onstartover } = $props();

  /** A password being made, not checked. */
  let making = $derived(isNew || resetting);

  function submit(event) {
    event.preventDefault();
    onsubmit();
  }
</script>

<form onsubmit={submit} class="flex flex-col gap-3">
  {#if isNew}
    <h2>Create a password</h2>
    <p>A password is optional. With one, signing in asks for it after the emailed code. You can make one later from your account.</p>
  {:else if resetting}
    <h2>Reset your password</h2>
    <p>The code proved this is your email. Choose a new password to finish signing in; the old one stops working.</p>
  {:else}
    <p>Enter your password to finish.</p>
  {/if}
  <input
    bind:value={password}
    type="password"
    autocomplete={making ? "new-password" : "current-password"}
    minlength={making ? 12 : undefined}
    class="input w-full"
    aria-label="Password"
    placeholder={making ? "Password, 12 characters or more" : "Password"}
    required
  />
  {#if error}
    <p class="error">{error}</p>
  {/if}
  <div class="flex flex-wrap items-center gap-3">
    <button type="submit" class="btn btn-primary" disabled={busy}>{isNew ? "Create password" : resetting ? "Reset password" : "Continue"}</button>
    {#if isNew}
      <button type="button" class="btn" onclick={onskip} disabled={busy} data-skip>Skip</button>
    {:else}
      {#if onforgot && !resetting}
        <button type="button" class="link text-sm" onclick={onforgot} disabled={busy}>Forgot your password?</button>
      {/if}
      <button type="button" class="link text-sm" onclick={onstartover} disabled={busy}>Start over</button>
    {/if}
  </div>
</form>
