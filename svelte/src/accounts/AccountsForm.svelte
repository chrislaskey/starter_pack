<!-- The email and password fields, for the Sign up and Sign in pages,
     with the name field on Sign up, and the error the attempt came
     back with. -->
<script>
  /**
   * @typedef {Object} Props
   * @property {string} email
   * @property {string} password
   * @property {string} [name] bound on the Sign up page, which shows its field
   * @property {boolean} [withName]
   * @property {string} submitText
   * @property {"new-password" | "current-password"} passwordAutocomplete
   * @property {string} [error]
   * @property {boolean} [busy]
   * @property {() => void} onsubmit
   */

  /** @type {Props} */
  let { email = $bindable(""), password = $bindable(""), name = $bindable(""), withName = false, submitText, passwordAutocomplete, error = "", busy = false, onsubmit } = $props();

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
  <input
    bind:value={password}
    type="password"
    autocomplete={passwordAutocomplete}
    class="input w-full"
    aria-label="Password"
    placeholder="Password"
    required
  />
  {#if error}
    <p class="error">{error}</p>
  {/if}
  <div>
    <button type="submit" class="btn btn-primary" disabled={busy}>{submitText}</button>
  </div>
</form>
