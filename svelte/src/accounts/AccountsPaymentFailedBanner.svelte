<!-- Shown on every page when a plan's renewal could not be paid: the
     plan gives nothing until it is, see docs/design/purchasing.md, P10.
     The plan's page says where to fix it: Stripe's portal, or the store
     it was bought in, see docs/design/purchasing-phones.md, M6. -->
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
  <span class="grow">Your plan's payment failed, so your plan is paused. Update your payment to restore it.</span>
  <a href={accountsSubscriptionsShowUrl(subscriptionId)} class="btn btn-sm">Update your payment</a>
</div>
