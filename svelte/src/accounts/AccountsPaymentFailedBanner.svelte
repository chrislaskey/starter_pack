<!-- Shown on every page when a plan's renewal could not be paid: the
     plan gives nothing until it is, see docs/design/purchasing.md, P10. -->
<script>
  import { accountsSubscriptionsShowUrl } from "../lib/routes.js";

  /**
   * @typedef {Object} Props
   * @property {string} subscriptionId the subscription whose payment failed
   */

  /** @type {Props} */
  let { subscriptionId } = $props();
</script>

<div class="alert alert-error flex flex-wrap items-center gap-3" role="status" data-payment-failed-banner>
  <span class="grow">Your plan's payment failed, so your plan is paused. Update your card to restore it.</span>
  <a href={accountsSubscriptionsShowUrl(subscriptionId)} class="btn btn-sm">Update your card</a>
</div>
