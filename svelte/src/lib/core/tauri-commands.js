// app_core over the Tauri app's commands (../../../../tauri/src/).
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { FormError } from "./form_error.js";

/** The user signed in on this device; an error when nobody is. */
export function usersMe() {
  return invoke("users_me");
}

/**
 * A form's command, whose error is `{error, errors}`, thrown as the
 * browser's would be.
 * @param {string} command
 * @param {Record<string, unknown>} args
 */
async function invokeForm(command, args) {
  try {
    return await invoke(command, args);
  } catch (problem) {
    throw formErrorOf(problem);
  }
}

/**
 * A command's `{error, errors}` as a FormError; anything else as it is.
 * @param {unknown} problem
 */
function formErrorOf(problem) {
  if (problem && typeof problem === "object" && "error" in problem) {
    const { error, errors } = /** @type {{error: string, errors?: Record<string, string[]>}} */ (problem);
    return new FormError(error, errors);
  }
  return problem;
}

/**
 * Where to PUT the picture the person picked, asked of the web server.
 * @param {string} contentType
 * @param {number} size in bytes
 * @returns {Promise<{upload_url: string, content_type: string, key: string}>}
 */
export function avatarsCreateUpload(contentType, size) {
  return invokeForm("avatars_create_upload", { contentType, size });
}

/**
 * Makes the uploaded file the user's picture, on the web server, and
 * here at once; answers the user.
 * @param {string} key
 */
export function avatarsUpdate(key) {
  return invokeForm("avatars_update", { key });
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

/** The user's files on this device, each `{file, shown_as, state, error}`. */
export function filesList() {
  return invoke("files_list");
}

/** @param {string} id */
export function filesGet(id) {
  return invoke("files_get", { id });
}

/**
 * @param {string} id
 * @param {string} name
 */
export function filesUpdate(id, name) {
  return invokeForm("files_update", { id, name });
}

/** @param {string} id */
export function filesDelete(id) {
  return invoke("files_delete", { id });
}

/**
 * Adds the file the person picked: kept on this device at once, and
 * sent to the server by the next sync; answers the file. The bytes are
 * the command's body, the name and the type its headers.
 * @param {File} file
 * @param {string} name
 * @param {(fraction: number) => void} [_onProgress] nothing to wait for here
 */
export async function filesCreate(file, name, _onProgress) {
  const bytes = new Uint8Array(await file.arrayBuffer());
  const headers = { "x-file-name": encodeURIComponent(name), "x-file-content-type": file.type };
  try {
    return await invoke("files_create", bytes, { headers });
  } catch (problem) {
    throw formErrorOf(problem);
  }
}

/** Whether this device saves a copy of a file outside the app. */
export function filesDownloadSupported() {
  return invoke("files_download_supported");
}

/**
 * Saves a copy of the file in the Downloads folder.
 * @param {string} id
 * @returns {Promise<string | null>} where it was saved
 */
export function filesDownload(id) {
  return invoke("files_download", { id });
}

/**
 * Where a page reads the file's bytes from: the app's `files` protocol,
 * which serves the copy on this device.
 * @param {string} id
 */
export function filesContentUrl(id) {
  return convertFileSrc(id, "files");
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

/**
 * Sends a refused change again as it was.
 * @param {string} id
 */
export function conflictsTryAgain(id) {
  return invoke("conflicts_try_again", { id });
}

/** @param {string} id */
export function conflictsLetGo(id) {
  return invoke("conflicts_let_go", { id });
}
