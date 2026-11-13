<!-- Inside the app: whether it is working offline, what waits to reach
     the server, the changes it did not keep, and the buttons to sync now
     and to work offline or go online. The web shows nothing: it is the
     server; and nothing for a local user, who has nothing to sync with.
     A sync that found the account deleted elsewhere, which removed it
     from the device, opens the device's accounts saying so. -->
<script>
  import { onSynced, syncRun, syncSetOffline, syncStatus } from "./core/index.js";
  import { currentUserForget } from "./current_user.js";
  import { navigateToUrl } from "./router.js";
  import { accountsIndexUrl, conflictsIndexUrl } from "./routes.js";

  let status = $state(null);
  let busy = $state(false);
  let error = $state("");

  async function load() {
    try {
      status = await syncStatus();
    } catch (problem) {
      error = String(problem);
    }
  }

  /** @param {unknown} outcome */
  function synced(outcome) {
    const {
      report,
      error: failure,
      account_deleted: accountDeleted,
    } = /** @type {{report?: {push_error?: string}, error?: string, account_deleted?: boolean}} */ (outcome ?? {});
    if (accountDeleted) {
      currentUserForget();
      navigateToUrl(accountsIndexUrl({ accountDeleted: true }));
      return;
    }
    error = failure ?? report?.push_error ?? "";
    load();
  }

  /** @param {() => Promise<unknown>} action */
  async function doThen(action) {
    busy = true;
    error = "";
    try {
      await action();
      await load();
    } catch (problem) {
      error = String(problem);
    } finally {
      busy = false;
    }
  }

  $effect(() => onSynced(synced));
  load();
</script>

{#if status && status.signed_in && !status.local}
  <div class="flex flex-wrap items-center gap-3 text-sm">
    {#if status.offline}
      <span class="badge badge-warning badge-sm">Working offline</span>
    {/if}
    {#if status.conflicts > 0}
      <a href={conflictsIndexUrl()} class="link text-warning">
        {status.conflicts}
        {status.conflicts === 1 ? "change was" : "changes were"} not saved on the server
      </a>
    {/if}
    {#if status.pending > 0}
      <span class="muted">{status.pending} waiting to sync</span>
    {/if}
    {#if status.offline}
      <button type="button" class="btn btn-xs btn-ghost" onclick={() => doThen(() => syncSetOffline(false))} disabled={busy}>
        Go online
      </button>
    {:else}
      <button type="button" class="btn btn-xs btn-ghost" onclick={() => doThen(syncRun)} disabled={busy}>Sync now</button>
      <button type="button" class="btn btn-xs btn-ghost" onclick={() => doThen(() => syncSetOffline(true))} disabled={busy}>
        Work offline
      </button>
    {/if}
    {#if error}
      <span class="error">{error}</span>
    {/if}
  </div>
{/if}
