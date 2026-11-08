<!-- Signing up with an email and a password, at /accounts/sign-up; done,
     the person is signed in and sees the posts -->
<script>
  import AppNav from "../lib/AppNav.svelte";
  import { accountsSignUp } from "../lib/core/index.js";
  import { forgetCurrentUser } from "../lib/current_user.js";
  import { navigateToUrl } from "../lib/router.js";
  import { accountsSignInUrl, postsIndexUrl } from "../lib/routes.js";
  import AccountsForm from "./AccountsForm.svelte";

  let email = $state("");
  let password = $state("");
  let error = $state("");
  let busy = $state(false);

  async function signUp() {
    busy = true;
    error = "";
    try {
      await accountsSignUp(email, password);
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
  <h1>Sign up</h1>
  <AccountsForm bind:email bind:password submitText="Sign up" passwordAutocomplete="new-password" {error} {busy} onsubmit={signUp} />
  <p class="muted">Already have an account? <a href={accountsSignInUrl()} class="link">Sign in</a></p>
</main>
