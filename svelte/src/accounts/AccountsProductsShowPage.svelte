<!-- One product for sale, at /accounts/products/:id: what it is, and a
     Buy button per price, which goes to Stripe's page to pay, or on a
     phone opens its store's sheet, with the store's price, the plan's
     length and the terms and privacy links Apple asks for. Someone
     with no account yet is asked to sign up; a one-time product the
     person has is Owned, unless it can be bought again, and someone on
     a plan does not buy another: they switch, in the store the plan is
     from. See docs/design/purchasing.md, P1, P2 and P4, and
     docs/design/purchasing-phones.md, M1, M4 and M5. -->
<script>
  import AppNav from "../lib/AppNav.svelte";
  import PublicPagesLinks from "../lib/PublicPagesLinks.svelte";
  import AccountsNeeded from "./AccountsNeeded.svelte";
  import {
    accountsCheckoutsCreate,
    accountsProductsGet,
    accountsStoreTransactionsCreate,
    accountsSubscriptionsUpdate,
  } from "../lib/core/index.js";
  import { currentUserLoad, currentUserReload } from "../lib/current_user.js";
  import { navigateToUrl, pathParams } from "../lib/router.js";
  import {
    accountsCheckoutsShowUrl,
    accountsProductsIndexUrl,
    accountsPurchasesShowUrl,
    accountsSubscriptionsShowUrl,
  } from "../lib/routes.js";
  import { priceFormat } from "./money.js";
  import { storeManagedWhere, storeName, storePriceFormat } from "./stores.js";

  const id = pathParams().id;
  let product = $state();
  /** @type {import("../lib/current_user.js").User | null} */
  let currentUser = $state(null);
  let buying = $state(false);
  let pending = $state(false);
  let error = $state("");
  /** Where this app sells: Stripe, or the phone's store. */
  const store = $derived(product?.store ?? "stripe");

  Promise.all([accountsProductsGet(id), currentUserLoad()]).then(
    ([shown, user]) => {
      product = shown;
      currentUser = user;
    },
    (problem) => (error = String(problem)),
  );

  /** @param {string} priceId */
  async function planSwitch(priceId) {
    const plan = currentUser?.plan;
    if (!plan) return;
    buying = true;
    error = "";
    try {
      await accountsSubscriptionsUpdate(plan.subscription_id, { action: "switch", price_id: priceId });
      await currentUserReload();
      await navigateToUrl(accountsSubscriptionsShowUrl(plan.subscription_id));
    } catch (problem) {
      error = String(problem);
      buying = false;
    }
  }

  /** @param {string} priceId */
  async function buy(priceId) {
    if (store !== "stripe") return storeBuy(priceId);
    buying = true;
    error = "";
    try {
      const { checkout, url } = await accountsCheckoutsCreate(priceId);
      // In the app Stripe's page opens in the browser, and the checkout's page waits
      if (url) window.location.assign(url);
      else await navigateToUrl(accountsCheckoutsShowUrl(checkout.id));
    } catch (problem) {
      error = String(problem);
      buying = false;
    }
  }

  /**
   * The store's sheet; a switch is buying the other plan there, and the
   * store decides when it starts.
   * @param {string} priceId
   */
  async function storeBuy(priceId) {
    buying = true;
    pending = false;
    error = "";
    try {
      const bought = await accountsStoreTransactionsCreate(priceId);
      if (bought.pending) pending = true;
      if (bought.purchase || bought.subscription) await currentUserReload();
      if (bought.purchase) return navigateToUrl(accountsPurchasesShowUrl(bought.purchase.id));
      if (bought.subscription) return navigateToUrl(accountsSubscriptionsShowUrl(bought.subscription.id));
    } catch (problem) {
      error = String(problem);
    }
    buying = false;
  }
</script>

<main class="page">
  <AppNav />

  {#if error}
    <p class="error">{error}</p>
  {/if}
  {#if product}
    <h1>{product.name}</h1>
    {#if product.description}
      <p>{product.description}</p>
    {/if}
    {#if product.free_days}
      <p class="muted">The first {product.free_days} days are free.</p>
    {/if}
    {#if currentUser && currentUser.kind !== "full"}
      <ul>
        {#each product.prices as price (price.id)}
          <li>{storePriceFormat(price, priceFormat)}</li>
        {/each}
      </ul>
      <AccountsNeeded kind={currentUser.kind} />
    {:else if product.kind === "plan" && currentUser?.plan}
      {@const plan = currentUser.plan}
      <p>You are on {plan.product_name}.</p>
      {#if plan.store !== store}
        <p class="muted">It was bought in {storeName(plan.store)}: manage it {storeManagedWhere(plan.store)}.</p>
      {:else if plan.cancel_at_period_end}
        <p class="muted">It ends with this period. Resume it on its page to switch.</p>
      {:else}
        <div class="flex flex-wrap gap-2">
          {#each product.prices as price (price.id)}
            {#if price.id !== plan.price_id}
              <button
                type="button"
                class="btn btn-primary btn-sm"
                disabled={buying}
                onclick={() => (store === "stripe" ? planSwitch(price.id) : storeBuy(price.id))}
              >
                Switch · {storePriceFormat(price, priceFormat)}
              </button>
            {/if}
          {/each}
        </div>
        {#if store === "stripe"}
          <p class="muted">A dearer plan, or yearly, starts now with the difference charged; a cheaper one, or monthly, starts when this period ends.</p>
        {:else}
          <p class="muted">{storeName(store)} decides when the new plan starts: an upgrade at once, a downgrade at the renewal.</p>
        {/if}
      {/if}
      <p><a href={accountsSubscriptionsShowUrl(plan.subscription_id)} class="link">See your plan</a></p>
    {:else if product.owned && !product.is_repeatable}
      <p>Owned</p>
    {:else}
      {#if product.owned}
        <p class="muted">You have this. You can buy it again.</p>
      {/if}
      <div class="flex flex-wrap gap-2">
        {#each product.prices as price (price.id)}
          <button type="button" class="btn btn-primary btn-sm" disabled={buying} onclick={() => buy(price.id)}>
            Buy · {storePriceFormat(price, priceFormat)}
          </button>
        {/each}
      </div>
      {#if pending}
        <p class="muted">Waiting for approval. It appears here once {storeName(store)} confirms it.</p>
      {/if}
    {/if}
    {#if store !== "stripe" && product.kind === "plan"}
      <p class="muted text-sm">
        {product.name}: an auto-renewing subscription, renewed each {product.prices
          .map((price) => price.store_period ?? price.interval)
          .filter(Boolean)
          .join(" or ")} at the price shown until you cancel it in {storeName(store)}.
        By buying you accept the <PublicPagesLinks />.
      </p>
    {/if}
  {/if}

  <a href={accountsProductsIndexUrl()}>Back to plans and products</a>
</main>
