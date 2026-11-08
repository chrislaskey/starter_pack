// app_core over the Tauri app's commands (../../../../tauri/src/).
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/** The user signed in on this device; an error when nobody is. */
export function usersMe() {
  return invoke("users_me");
}

/**
 * @param {string} email
 * @param {string} password
 */
export function accountsSignUp(email, password) {
  return invoke("accounts_sign_up", { email, password });
}

/**
 * @param {string} email
 * @param {string} password
 */
export function accountsSignIn(email, password) {
  return invoke("accounts_sign_in", { email, password });
}

/**
 * Syncs what waits first, then signs out and drops the device's copy of
 * the data. Answers `{outcome: "signed_out"}`, or, when a change would be
 * lost and the person has not chosen to lose it,
 * `{outcome: "changes_would_be_lost", pending, conflicts}` with nothing done.
 * @param {boolean} losingChanges the person chose to sign out losing them
 */
export function accountsSignOut(losingChanges = false) {
  return invoke("accounts_sign_out", { losingChanges });
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
