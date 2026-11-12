<!-- The fields of a community, for the New and Edit pages: its name,
     bound to the page's `name`, with the messages a write came back with. -->
<script>
  import FieldErrors from "../lib/FieldErrors.svelte";

  /**
   * @typedef {Object} Props
   * @property {string} name
   * @property {string} submitText
   * @property {Record<string, string[]>} [errors] a FormError's, by field
   * @property {string} [error]
   * @property {boolean} [busy]
   * @property {() => void} onsubmit
   */

  /** @type {Props} */
  let { name = $bindable(""), submitText, errors = {}, error = "", busy = false, onsubmit } = $props();

  /** @param {SubmitEvent} event */
  function submit(event) {
    event.preventDefault();
    onsubmit();
  }
</script>

<form onsubmit={submit} class="flex flex-col gap-3">
  <label class="flex flex-col gap-1">
    <span>Name</span>
    <input type="text" class="input w-full" bind:value={name} maxlength="80" disabled={busy} />
    <FieldErrors {errors} fields={["name"]} />
  </label>
  {#if error && Object.keys(errors).length === 0}
    <p class="error">{error}</p>
  {/if}
  <div>
    <button type="submit" class="btn btn-primary" disabled={busy}>{submitText}</button>
  </div>
</form>
