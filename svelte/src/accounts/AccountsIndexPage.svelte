<!-- The accounts on this device, at /accounts, inside the app: the active
     one, the others to switch to or sign in to, each to remove from the
     device, and Add account. The device section of the app: no account
     is open while the person adds one, which happens in the browser,
     with the dialog waiting for its code. -->
<script>
  import AppNav from "../lib/AppNav.svelte";
  import {
    accountsSignInCancel,
    accountsSignInFinish,
    accountsSignInPending,
    accountsSignInStart,
    deviceAccountsDeactivate,
    deviceAccountsList,
    deviceAccountsRemove,
    deviceAccountsSwitch,
  } from "../lib/core/index.js";
  import { currentUserForget } from "../lib/current_user.js";
  import { navigateToUrl } from "../lib/router.js";
  import { postsIndexUrl } from "../lib/routes.js";
  import AccountsRemoveDialog from "./AccountsRemoveDialog.svelte";
  import AccountsSignInDialog from "./AccountsSignInDialog.svelte";

  /** @typedef {{user_id: string, email: string, signed_in: boolean, active: boolean, last_active_at: string}} DeviceAccount */

  /** @type {DeviceAccount[]} */
  let accounts = $state([]);
  /** @type {DeviceAccount | null} */
  let removing = $state(null);
  /** @type {{pending: number, conflicts: number} | null} */
  let loss = $state(null);
  /** @type {{email: string | null} | null} */
  let signingIn = $state(null);
  let signInError = $state("");
  let busy = $state(false);
  let error = $state("");

  async function load() {
    try {
      accounts = await deviceAccountsList();
      signingIn = await accountsSignInPending();
    } catch (problem) {
      error = String(problem);
    }
  }

  /** @param {() => Promise<unknown>} action */
  async function doThen(action) {
    busy = true;
    error = "";
    try {
      await action();
    } catch (problem) {
      error = String(problem);
    } finally {
      busy = false;
    }
  }

  /** @param {DeviceAccount} account */
  function switchTo(account) {
    return doThen(async () => {
      await deviceAccountsSwitch(account.user_id);
      currentUserForget();
      await navigateToUrl(postsIndexUrl());
    });
  }

  /**
   * Closes the active account and opens the browser to sign in, then
   * waits for the code in the dialog.
   * @param {string | null} email
   */
  function openBrowser(email) {
    return doThen(async () => {
      await deviceAccountsDeactivate();
      currentUserForget();
      signInError = "";
      await accountsSignInStart(email ?? undefined);
      signingIn = { email };
      accounts = await deviceAccountsList();
    });
  }

  /** @param {string} code */
  function finishSignIn(code) {
    return doThen(async () => {
      signInError = "";
      try {
        await accountsSignInFinish(code);
      } catch (problem) {
        signInError = String(problem);
        return;
      }
      signingIn = null;
      currentUserForget();
      await navigateToUrl(postsIndexUrl());
    });
  }

  function cancelSignIn() {
    return doThen(async () => {
      signingIn = null;
      signInError = "";
      await accountsSignInCancel();
    });
  }

  /** @param {boolean} losingChanges */
  function remove(losingChanges) {
    const account = removing;
    if (!account) return;
    return doThen(async () => {
      const answer = await deviceAccountsRemove(account.user_id, losingChanges);
      if (answer.outcome === "changes_would_be_lost") {
        loss = { pending: answer.pending, conflicts: answer.conflicts };
        return;
      }
      removing = null;
      loss = null;
      if (account.active) currentUserForget();
      await load();
    });
  }

  load();
</script>

<main class="page">
  <AppNav />
  <h1>Accounts on this device</h1>

  {#if error}
    <p class="error">{error}</p>
  {/if}

  {#if accounts.length === 0 && !error}
    <p class="muted">No account yet.</p>
  {/if}

  <ul class="posts">
    {#each accounts as account (account.user_id)}
      <li class="post flex flex-wrap items-center gap-3">
        <span class="grow">{account.email}</span>
        {#if account.active}
          <span class="badge badge-primary badge-sm">Active</span>
        {:else if account.signed_in}
          <button type="button" class="btn btn-sm" onclick={() => switchTo(account)} disabled={busy}>Switch</button>
        {:else}
          <span class="badge badge-ghost badge-sm">Signed out</span>
          <button type="button" class="btn btn-sm" onclick={() => openBrowser(account.email)} disabled={busy}>Sign in</button>
        {/if}
        <button type="button" class="btn btn-sm btn-ghost" onclick={() => ((loss = null), (removing = account))} disabled={busy}>
          Remove from device
        </button>
      </li>
    {/each}
  </ul>

  <button type="button" class="btn btn-sm" onclick={() => openBrowser(null)} disabled={busy}>Add account</button>

  <AccountsSignInDialog
    pending={signingIn}
    error={signInError}
    {busy}
    onfinish={finishSignIn}
    onopenbrowser={() => openBrowser(signingIn?.email ?? null)}
    oncancel={cancelSignIn}
  />

  <AccountsRemoveDialog
    account={removing}
    {loss}
    {busy}
    onremove={remove}
    oncancel={() => ((removing = null), (loss = null))}
  />
</main>
