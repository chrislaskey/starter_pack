<!-- Signing in, at /accounts/sign-in, on the web: the email and the
     password, then the code that was emailed; done, the person sees the
     posts. Opened by a device with ?challenge=, the code ends in a code
     for the device instead, see docs/design/accounts-sign-in.md. The
     email is filled in from ?email=. -->
<script>
  import AppNav from "../lib/AppNav.svelte";
  import { accountsCode, accountsCodeResend, accountsSignIn } from "../lib/core/index.js";
  import { currentUserForget } from "../lib/current_user.js";
  import { navigateToUrl } from "../lib/router.js";
  import { accountsSignUpUrl, postsIndexUrl } from "../lib/routes.js";
  import AccountsCodeForm from "./AccountsCodeForm.svelte";
  import AccountsDeviceCode from "./AccountsDeviceCode.svelte";
  import AccountsForm from "./AccountsForm.svelte";

  const query = new URLSearchParams(window.location.search);
  const challenge = query.get("challenge") ?? "";

  let email = $state(query.get("email") ?? "");
  let password = $state("");
  let pending = $state("");
  let code = $state("");
  let deviceCode = $state("");
  let resent = $state(false);
  let error = $state("");
  let busy = $state(false);

  /** @param {() => Promise<void>} action */
  async function attempt(action) {
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

  function signIn() {
    return attempt(async () => {
      const sent = await accountsSignIn(email, password);
      pending = sent.pending;
      password = "";
      code = "";
    });
  }

  function finish() {
    return attempt(async () => {
      const answer = await accountsCode(pending, code, challenge);
      if ("code" in answer) {
        deviceCode = answer.code;
        return;
      }
      currentUserForget();
      await navigateToUrl(postsIndexUrl());
    });
  }

  function resend() {
    return attempt(async () => {
      resent = false;
      await accountsCodeResend(pending);
      resent = true;
      code = "";
    });
  }

  function startOver() {
    pending = "";
    code = "";
    error = "";
    resent = false;
  }
</script>

<main class="page">
  <AppNav />
  <h1>Sign in</h1>
  {#if challenge && !deviceCode}
    <p class="muted">Signing in for the app on your device.</p>
  {/if}
  {#if deviceCode}
    <AccountsDeviceCode code={deviceCode} />
  {:else if pending}
    <AccountsCodeForm {email} bind:code {error} {busy} {resent} onsubmit={finish} onresend={resend} onstartover={startOver} />
  {:else}
    <AccountsForm bind:email bind:password submitText="Sign in" passwordAutocomplete="current-password" {error} {busy} onsubmit={signIn} />
    <p class="muted">New here? <a href={accountsSignUpUrl(email || undefined)} class="link">Sign up</a></p>
  {/if}
</main>
