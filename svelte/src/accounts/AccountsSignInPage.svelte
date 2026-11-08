<!-- Signing in with an email and a password, at /accounts/sign-in; done,
     the person sees the posts -->
<script>
  import AppNav from "../lib/AppNav.svelte";
  import { accountsSignIn } from "../lib/core/index.js";
  import { forgetCurrentUser } from "../lib/current_user.js";
  import { navigateToUrl } from "../lib/router.js";
  import { accountsSignUpUrl, postsIndexUrl } from "../lib/routes.js";
  import AccountsForm from "./AccountsForm.svelte";

  let email = $state("");
  let password = $state("");
  let error = $state("");
  let busy = $state(false);

  async function signIn() {
    busy = true;
    error = "";
    try {
      await accountsSignIn(email, password);
      forgetCurrentUser();
      await navigateToUrl(postsIndexUrl());
    } catch (problem) {
      error = String(problem);
    } finally {
      busy = false;
    }
  }
</script>

<main class="page">
  <AppNav />
  <h1>Sign in</h1>
  <AccountsForm bind:email bind:password submitText="Sign in" passwordAutocomplete="current-password" {error} {busy} onsubmit={signIn} />
  <p class="muted">New here? <a href={accountsSignUpUrl()} class="link">Sign up</a></p>
</main>
