<!-- The links between the pages, the current community to switch, the
     other apps (support, the forum, and ops for staff on the web), who
     is signed in, by their email, a link to their account, with Sign
     out, or Sign in and Sign up on the web and Accounts in the app, the
     theme menu, and inside the app the sync status. A local user has no
     email and no sign-out: "On this device only", to their profile. A
     trial user sees the trial's banner above every page, and has no
     sign-out either, having nothing to sign back in with. -->
<script>
  import AccountsPaymentFailedBanner from "../accounts/AccountsPaymentFailedBanner.svelte";
  import AccountsTrialBanner from "../accounts/AccountsTrialBanner.svelte";
  import CurrentCommunityMenu from "./CurrentCommunityMenu.svelte";
  import SyncStatus from "./SyncStatus.svelte";
  import ThemeMenu from "./ThemeMenu.svelte";
  import { accountsSignOut, hasDeviceAccounts } from "./core/index.js";
  import { currentUserForget, currentUserLoad } from "./current_user.js";
  import { navigateToUrl } from "./router.js";
  import {
    accountsIndexUrl,
    accountsSettingsUrl,
    accountsSignInUrl,
    accountsSignUpUrl,
    communitiesIndexUrl,
    filesIndexUrl,
    forumIndexUrl,
    homeUrl,
    opsHomeUrl,
    postsIndexUrl,
    profileEditUrl,
    supportIndexUrl,
  } from "./routes.js";

  /** @type {import("./current_user.js").User | null} */
  let currentUser = $state(null);
  let busy = $state(false);
  let error = $state("");

  currentUserLoad().then(
    (user) => (currentUser = user),
    () => (currentUser = null),
  );

  async function signOut() {
    error = "";
    busy = true;
    try {
      await accountsSignOut();
      currentUserForget();
      await navigateToUrl(hasDeviceAccounts ? accountsIndexUrl() : accountsSignInUrl());
    } catch (problem) {
      error = String(problem);
    } finally {
      busy = false;
    }
  }
</script>

{#if currentUser?.kind === "trial" && currentUser.trial_expires_at}
  <AccountsTrialBanner trialExpiresAt={currentUser.trial_expires_at} />
{/if}
{#if currentUser?.payment_failed_subscription_id}
  <AccountsPaymentFailedBanner subscriptionId={currentUser.payment_failed_subscription_id} />
{/if}
<nav class="flex flex-wrap items-center gap-4">
  <a href={homeUrl()} class="link">Home</a>
  {#if currentUser}
    <CurrentCommunityMenu />
    <a href={postsIndexUrl()} class="link">Posts</a>
    <a href={filesIndexUrl()} class="link">Files</a>
    <a href={communitiesIndexUrl()} class="link">Communities</a>
    <a href={profileEditUrl()} class="link">Profile</a>
    <a href={supportIndexUrl()} class="link">Support</a>
    <a href={forumIndexUrl()} class="link">Forum</a>
    {#if currentUser.is_staff}
      <!-- target, so the router leaves the link to the browser: ops is another entry -->
      <a href={opsHomeUrl()} target="_self" class="link">Ops</a>
    {/if}
  {/if}
  <span class="grow"></span>
  {#if currentUser}
    {#if currentUser.kind === "local"}
      <a href={profileEditUrl()} class="link text-sm">On this device only</a>
    {:else if currentUser.kind === "trial"}
      <a href={accountsSignUpUrl("")} class="link text-sm">Trial</a>
    {:else}
      <a href={accountsSettingsUrl()} class="link text-sm">{currentUser.email}</a>
    {/if}
    {#if hasDeviceAccounts}
      <a href={accountsIndexUrl()} class="link text-sm">Accounts</a>
    {/if}
    {#if currentUser.kind === "full"}
      <button type="button" class="btn btn-ghost btn-sm" onclick={signOut} disabled={busy}>Sign out</button>
    {/if}
  {:else if hasDeviceAccounts}
    <a href={accountsIndexUrl()} class="link">Accounts</a>
  {:else}
    <a href={accountsSignInUrl()} class="link">Sign in</a>
    <a href={accountsSignUpUrl()} class="link">Sign up</a>
  {/if}
  <ThemeMenu />
</nav>
{#if error}
  <p class="error">{error}</p>
{/if}
{#if currentUser}
  <SyncStatus />
{/if}
