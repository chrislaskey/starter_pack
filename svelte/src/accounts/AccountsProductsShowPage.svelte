<!-- One product for sale, at /accounts/products/:id: what it is, and a
     Buy button per price, which goes to Stripe's page to pay. Someone
     with no account yet is asked to sign up; a one-time product the
     person has is Owned, unless it can be bought again, and someone on
     a plan does not buy another. See docs/design/purchasing.md, P1, P2
     and P4. -->
<script>
  import AppNav from "../lib/AppNav.svelte";
  import AccountsNeeded from "./AccountsNeeded.svelte";
  import { accountsCheckoutsCreate, accountsProductsGet } from "../lib/core/index.js";
  import { currentUserLoad } from "../lib/current_user.js";
  import { pathParams } from "../lib/router.js";
  import { accountsProductsIndexUrl, accountsSubscriptionsShowUrl } from "../lib/routes.js";
  import { priceFormat } from "./money.js";

  const id = pathParams().id;
  let product = $state();
  /** @type {import("../lib/current_user.js").User | null} */
  let currentUser = $state(null);
  let buying = $state(false);
  let error = $state("");

  Promise.all([accountsProductsGet(id), currentUserLoad()]).then(
    ([shown, user]) => {
      product = shown;
      currentUser = user;
    },
    (problem) => (error = String(problem)),
  );

  /** @param {string} priceId */
  async function buy(priceId) {
    buying = true;
    error = "";
    try {
      const { url } = await accountsCheckoutsCreate(priceId);
      window.location.assign(url);
    } catch (problem) {
      error = String(problem);
      buying = false;
    }
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
          <li>{priceFormat(price)}</li>
        {/each}
      </ul>
      <AccountsNeeded kind={currentUser.kind} />
    {:else if product.kind === "plan" && currentUser?.plan}
      {#if currentUser.plan.product_id === product.id}
        <p>This is your plan.</p>
      {:else}
        <p>You are on {currentUser.plan.product_name}.</p>
      {/if}
      <p><a href={accountsSubscriptionsShowUrl(currentUser.plan.subscription_id)} class="link">See your plan</a></p>
    {:else if product.owned && !product.is_repeatable}
      <p>Owned</p>
    {:else}
      {#if product.owned}
        <p class="muted">You have this. You can buy it again.</p>
      {/if}
      <div class="flex flex-wrap gap-2">
        {#each product.prices as price (price.id)}
          <button type="button" class="btn btn-primary btn-sm" disabled={buying} onclick={() => buy(price.id)}>
            Buy · {priceFormat(price)}
          </button>
        {/each}
      </div>
    {/if}
  {/if}

  <a href={accountsProductsIndexUrl()}>Back to plans and products</a>
</main>
