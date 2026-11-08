// app_core over the Tauri app's commands (../../../../tauri/src/).
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/** The user signed in on this device; an error when nobody is. */
export function usersMe() {
  return invoke("users_me");
}

// The app never asks for a password: a sign-in happens in the system
// browser, on the web's sign-in page, which ends by showing a code the
// person pastes into the app, see accountsSignInStart.

/** @param {string} _email @param {string} _password */
export function accountsSignUp(_email, _password) {
  return Promise.reject(new Error("in the app, sign in from the accounts page"));
}

/** @param {string} _email @param {string} _password */
export function accountsSignIn(_email, _password) {
  return Promise.reject(new Error("in the app, sign in from the accounts page"));
}

/** @param {string} _pending @param {string} _code @param {string} [_challenge] */
export function accountsCode(_pending, _code, _challenge) {
  return Promise.reject(new Error("in the app, sign in from the accounts page"));
}

/** @param {string} _pending */
export function accountsCodeResend(_pending) {
  return Promise.reject(new Error("in the app, sign in from the accounts page"));
}

/**
 * Opens the browser on the web's sign-in page, with the email filled in
 * for an account signed out on this device, and waits for its code.
 * @param {string} [email]
 */
export function accountsSignInStart(email) {
  return invoke("accounts_sign_in_start", { email: email || null });
}

/** The sign-in waiting on the browser, `{email, started_at}`, or null. */
export function accountsSignInPending() {
  return invoke("accounts_sign_in_pending");
}

/**
 * Exchanges the code the browser showed for the session, which becomes
 * the active account; answers the user.
 * @param {string} code
 */
export function accountsSignInFinish(code) {
  return invoke("accounts_sign_in_finish", { code });
}

export function accountsSignInCancel() {
  return invoke("accounts_sign_in_cancel");
}

/**
 * Syncs what waits first, then signs out and closes the account, whose
 * data stays on the device for the next sign-in.
 */
export function accountsSignOut() {
  return invoke("accounts_sign_out");
}

/** The app keeps the accounts signed in on this device, see deviceAccountsList. */
export const hasDeviceAccounts = true;

export function deviceAccountsList() {
  return invoke("device_accounts_list");
}

/**
 * Makes a signed-in account the active one, which then syncs.
 * @param {string} userId
 */
export function deviceAccountsSwitch(userId) {
  return invoke("device_accounts_switch", { userId });
}

/** Closes the active account, which stays signed in, before adding another. */
export function deviceAccountsDeactivate() {
  return invoke("device_accounts_deactivate");
}

/**
 * Removes an account and its data from this device. Answers
 * `{outcome: "removed"}`, or, when a change would be lost and the person
 * has not chosen to lose it, `{outcome: "changes_would_be_lost", pending,
 * conflicts}` with nothing done.
 * @param {string} userId
 * @param {boolean} losingChanges
 */
export function deviceAccountsRemove(userId, losingChanges) {
  return invoke("device_accounts_remove", { userId, losingChanges });
}

export function postsList() {
  return invoke("posts_list");
}

/** @param {string} id */
export function postsGet(id) {
  return invoke("posts_get", { id });
}

/**
 * @param {string} body
 * @param {string[]} communityIds the communities the post is in
 */
export function postsCreate(body, communityIds) {
  return invoke("posts_create", { body, communityIds });
}

/**
 * @param {string} id
 * @param {string} body
 * @param {string[]} communityIds the communities the post is in, exactly
 */
export function postsUpdate(id, body, communityIds) {
  return invoke("posts_update", { id, body, communityIds });
}

/** Every community, each with the user's membership when they are in it. */
export function communitiesList() {
  return invoke("communities_list");
}

/** @param {string} name */
export function communitiesCreate(name) {
  return invoke("communities_create", { name });
}

/** @param {string} communityId */
export function membershipsJoin(communityId) {
  return invoke("memberships_join", { communityId });
}

/** @param {string} communityId */
export function membershipsLeave(communityId) {
  return invoke("memberships_leave", { communityId });
}

/** @param {string} id */
export function postsDelete(id) {
  return invoke("posts_delete", { id });
}

/** One sync now: the report, or the error as the message. */
export function syncRun() {
  return invoke("sync_run");
}

/**
 * Works offline, or online again, which syncs at once.
 * @param {boolean} offline
 */
export function syncSetOffline(offline) {
  return invoke("sync_set_offline", { offline });
}

/** Whether working offline, what waits, what conflicted, where the pull is. */
export function syncStatus() {
  return invoke("sync_status");
}

/**
 * Calls back after every sync with `{report, error}`.
 * @param {(outcome: unknown) => void} callback
 * @returns {() => void} stops listening
 */
export function onSynced(callback) {
  const stopping = listen("synced", (event) => callback(event.payload));
  return () => {
    stopping.then((stop) => stop());
  };
}

export function conflictsList() {
  return invoke("conflicts_list");
}

export function pendingChangesList() {
  return invoke("pending_changes_list");
}

/** @param {string} id */
export function conflictsReapply(id) {
  return invoke("conflicts_reapply", { id });
}

/** @param {string} id */
export function conflictsLetGo(id) {
  return invoke("conflicts_let_go", { id });
}
