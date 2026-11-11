<!-- The email field, for the Sign up and Sign in pages, with the name
     field on Sign up, and the error the attempt came back with. -->
<script>
  /**
   * @typedef {Object} Props
   * @property {string} email
   * @property {string} [name] bound on the Sign up page, which shows its field
   * @property {boolean} [withName]
   * @property {string} submitText
   * @property {string} [error]
   * @property {boolean} [busy]
   * @property {() => void} onsubmit
   */

  /** @type {Props} */
  let { email = $bindable(""), name = $bindable(""), withName = false, submitText, error = "", busy = false, onsubmit } = $props();

  function submit(event) {
    event.preventDefault();
    onsubmit();
  }
</script>

<form onsubmit={submit} class="flex flex-col gap-3">
  {#if withName}
    <input bind:value={name} type="text" autocomplete="name" class="input w-full" aria-label="Name" placeholder="Name" maxlength="80" required />
  {/if}
  <input bind:value={email} type="email" autocomplete="email" class="input w-full" aria-label="Email" placeholder="Email" required />
  {#if error}
    <p class="error">{error}</p>
  {/if}
  <p class="muted text-sm">We will email you a code to continue.</p>
  <div>
    <button type="submit" class="btn btn-primary" disabled={busy}>{submitText}</button>
  </div>
</form>
