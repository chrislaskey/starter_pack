<!-- Shown in place of what needs an account on the server, to someone
     who has none yet: a local user, in the app, or a trial user, on the
     web. See docs/design/accounts-local-and-trial-users.md. -->
<script>
  import { accountsIndexUrl, accountsSignUpUrl } from "../lib/routes.js";

  /**
   * @typedef {Object} Props
   * @property {"local" | "trial"} kind
   */

  /** @type {Props} */
  let { kind } = $props();
</script>

{#if kind === "local"}
  <p>This is online only, and your account is only on this device.</p>
  <p><a href={accountsIndexUrl()} class="btn btn-sm btn-primary">Sign up or sign in</a></p>
{:else}
  <p>This needs an account, and you are on a trial.</p>
  <p><a href={accountsSignUpUrl()} class="btn btn-sm btn-primary">Sign up</a></p>
{/if}
