<!-- The messages for a form's fields that were wrong, under the input
     they belong to, as Phoenix's <.error> is: `errors` is a FormError's,
     and `fields` the names whose messages show here. -->
<script>
  /**
   * @typedef {Object} Props
   * @property {Record<string, string[]>} errors
   * @property {string[]} fields
   */

  /** @type {Props} */
  let { errors, fields } = $props();

  let messages = $derived(fields.flatMap((field) => errors[field] ?? []));
</script>

{#each messages as message (message)}
  <p class="error text-sm">{message}</p>
{/each}
