<!-- The fields of a post, for the New and Edit pages: the body, bound to
     the page's `text`, and the error the write came back with. -->
<script>
  /**
   * @typedef {Object} Props
   * @property {string} text
   * @property {string} submitText
   * @property {string} [error]
   * @property {boolean} [busy]
   * @property {() => void} onsubmit
   */

  /** @type {Props} */
  let { text = $bindable(""), submitText, error = "", busy = false, onsubmit } = $props();

  function submit(event) {
    event.preventDefault();
    onsubmit();
  }
</script>

<form onsubmit={submit} class="flex flex-col gap-3">
  <textarea bind:value={text} rows="4" class="textarea w-full" aria-label="Post" placeholder="What's on your mind?"
  ></textarea>
  {#if error}
    <p class="error">{error}</p>
  {/if}
  <div>
    <button type="submit" class="btn btn-primary" disabled={busy}>{submitText}</button>
  </div>
</form>
