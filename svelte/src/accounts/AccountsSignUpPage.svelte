<!-- Signing up, at /accounts/sign-up, on the web: the name, the email
     and the password, then the code that was emailed, which proves the email;
     done, the person is signed in and sees the posts. Opened by a device
     with ?challenge=, the code ends in a code for the device instead; the
     email is filled in from ?email=. -->
<script>
  import AppNav from "../lib/AppNav.svelte";
  import { accountsCode, accountsCodeResend, accountsSignUp } from "../lib/core/index.js";
  import { currentUserForget } from "../lib/current_user.js";
  import { navigateToUrl } from "../lib/router.js";
  import { accountsSignInUrl, postsIndexUrl } from "../lib/routes.js";
  import AccountsCodeForm from "./AccountsCodeForm.svelte";
  import AccountsDeviceCode from "./AccountsDeviceCode.svelte";
  import AccountsForm from "./AccountsForm.svelte";

  const query = new URLSearchParams(window.location.search);
  const challenge = query.get("challenge") ?? "";

  let email = $state(query.get("email") ?? "");
  let password = $state("");
  let name = $state("");
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

  function signUp() {
    return attempt(async () => {
      const sent = await accountsSignUp(email, password, name);
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
  <h1>Sign up</h1>
  {#if challenge && !deviceCode}
    <p class="muted">Signing up for the app on your device.</p>
  {/if}
  {#if deviceCode}
    <AccountsDeviceCode code={deviceCode} />
  {:else if pending}
    <AccountsCodeForm {email} bind:code {error} {busy} {resent} onsubmit={finish} onresend={resend} onstartover={startOver} />
  {:else}
    <AccountsForm bind:email bind:password bind:name withName submitText="Sign up" passwordAutocomplete="new-password" {error} {busy} onsubmit={signUp} />
    <p class="muted">Already have an account? <a href={accountsSignInUrl(email || undefined)} class="link">Sign in</a></p>
  {/if}
</main>
