<!-- Deleting the signed-in person's account, at /accounts/deletions/new:
     what goes, the plan's warning by where it was bought, Send code, the
     code and Delete my account, see docs/design/accounts-delete.md. A
     trial is deleted with no code. After it, the start page: the
     public home on the web, the device's accounts in the app. A local
     user has no account; removing the device's data is the accounts
     list's. -->
<script>
  import AppNav from "../lib/AppNav.svelte";
  import { accountsDeletionsCodeCreate, accountsDeletionsCreate, hasDeviceAccounts } from "../lib/core/index.js";
  import { currentUserForget, currentUserLoad } from "../lib/current_user.js";
  import { navigateToUrl } from "../lib/router.js";
  import { PUBLIC_HOME_PATH, accountsIndexUrl, accountsSettingsUrl } from "../lib/routes.js";

  /** Where each store's own subscriptions page is, to cancel a plan there. */
  const STORE_SUBSCRIPTIONS_URLS = {
    app_store: "https://apps.apple.com/account/subscriptions",
    play_store: "https://play.google.com/store/account/subscriptions",
  };

  /** @type {import("../lib/current_user.js").User | null} */
  let currentUser = $state(null);
  let codeSent = $state(false);
  let code = $state("");
  let busy = $state(false);
  let error = $state("");

  currentUserLoad().then(
    (user) => (currentUser = user),
    (problem) => (error = String(problem)),
  );

  /** @param {() => Promise<void>} action */
  async function doing(action) {
    busy = true;
    error = "";
    try {
      await action();
    } catch (problem) {
      error = problem instanceof Error ? problem.message : String(problem);
    } finally {
      busy = false;
    }
  }

  function sendCode() {
    return doing(async () => {
      await accountsDeletionsCodeCreate();
      codeSent = true;
    });
  }

  /** @param {SubmitEvent} event */
  function deleteAccount(event) {
    event.preventDefault();
    return doing(async () => {
      await accountsDeletionsCreate(currentUser?.kind === "trial" ? null : code.trim());
      currentUserForget();
      if (hasDeviceAccounts) await navigateToUrl(accountsIndexUrl({ accountDeleted: true }));
      else window.location.assign(PUBLIC_HOME_PATH);
    });
  }
</script>

<main class="page">
  <AppNav />
  <h1>Delete my account</h1>
  {#if currentUser?.kind === "local"}
    <p>This account is only on this device, with nothing on the server to delete.</p>
    <p>To remove it and its data, remove it from <a href={accountsIndexUrl()} class="link">this device's accounts</a>.</p>
  {:else if currentUser}
    <p>
      Deleting your account deletes everything you made, in every community and on every device, and your
      purchases. It cannot be undone.
    </p>
    {#if currentUser.plan?.store === "stripe"}
      <p class="text-warning">Your plan, {currentUser.plan.product_name}, ends now, with no refund.</p>
    {:else if currentUser.plan?.store === "app_store" || currentUser.plan?.store === "play_store"}
      {@const store = currentUser.plan.store === "app_store" ? "Apple" : "Google Play"}
      <p class="text-warning">
        Your plan, {currentUser.plan.product_name}, is billed by {store} until you cancel it there. Deleting your
        account does not cancel it.
        <a href={STORE_SUBSCRIPTIONS_URLS[currentUser.plan.store]} class="link" target="_blank" rel="noopener">
          Manage it in {currentUser.plan.store === "app_store" ? "the App Store" : "Google Play"}
        </a>
      </p>
    {/if}
    <form onsubmit={deleteAccount} class="flex flex-col gap-3">
      {#if currentUser.kind === "full"}
        <p>We email you a code to make sure it is you.</p>
        <div>
          <button type="button" class="btn" onclick={sendCode} disabled={busy}>{codeSent ? "Send a new code" : "Send code"}</button>
        </div>
        {#if codeSent}
          <p class="muted">A code is on its way to {currentUser.email}. It expires in ten minutes.</p>
          <input
            bind:value={code}
            inputmode="numeric"
            autocomplete="one-time-code"
            class="input w-full"
            aria-label="Code"
            placeholder="The code from the email"
            required
          />
        {/if}
      {/if}
      {#if error}
        <p class="error">{error}</p>
      {/if}
      <div class="flex flex-wrap items-center gap-3">
        {#if currentUser.kind === "trial" || codeSent}
          <button type="submit" class="btn btn-error" disabled={busy}>Delete my account</button>
        {/if}
        <a href={accountsSettingsUrl()} class="link text-sm">Back to account</a>
      </div>
    </form>
  {:else if error}
    <p class="error">{error}</p>
  {/if}
</main>
