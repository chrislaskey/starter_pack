// app_core over the Tauri app's commands (../../../../tauri/src/).
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { FormError } from "./form_error.js";

/** The user signed in on this device; null when nobody is. */
export function currentUserGet() {
  return invoke("current_user_get");
}

/**
 * The signed-in user's new name, written on this device and synced when it next can.
 * @param {string} name
 */
export function currentUserUpdate(name) {
  return invoke("current_user_update", { name });
}

/** The community the person works in on this device; null when they are in none. */
export function currentCommunityGet() {
  return invoke("current_community_get");
}

/** @param {string} communityId one the person is a member of */
export function currentCommunityUpdate(communityId) {
  return invoke("current_community_update", { communityId });
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

// The app asks for no password at a sign-in: it happens in the system
// browser, on the web's sign-in page, which ends by showing a code the
// person pastes into the app, see accountsSignInStart.

/** @param {string} _email @param {string} _name */
export function accountsSignUp(_email, _name) {
  return Promise.reject(new Error("in the app, sign in from the accounts page"));
}

/** @param {string} _email */
export function accountsSignIn(_email) {
  return Promise.reject(new Error("in the app, sign in from the accounts page"));
}

/** @param {string} _pending @param {string} _code @param {string} [_challenge] */
export function accountsCode(_pending, _code, _challenge) {
  return Promise.reject(new Error("in the app, sign in from the accounts page"));
}

/** @param {string} _pending @param {string} _password @param {string} [_challenge] */
export function accountsSignInPassword(_pending, _password, _challenge) {
  return Promise.reject(new Error("in the app, sign in from the accounts page"));
}

/** @param {string} _pending @param {string} _password @param {string} [_challenge] */
export function accountsSignInResetPassword(_pending, _password, _challenge) {
  return Promise.reject(new Error("in the app, sign in from the accounts page"));
}

/** @param {string} _pending @param {string | null} _password @param {string} [_challenge] */
export function accountsSignInNewPassword(_pending, _password, _challenge) {
  return Promise.reject(new Error("in the app, sign in from the accounts page"));
}

/**
 * What the signed-in person's account asks for after the emailed code,
 * asked of the server, so only when online.
 * @returns {Promise<Array<"password">>}
 */
export function accountsSecondFactors() {
  return invoke("accounts_second_factors");
}

/**
 * The signed-in person's password, kept by the server, so only when online.
 * @param {string | null} current
 * @param {string} password
 */
export function accountsPasswordUpdate(current, password) {
  return invoke("accounts_password_update", { current, password });
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

/**
 * The copies this device keeps of files the server holds too.
 * @returns {Promise<{count: number, bytes: number} | null>}
 */
export function filesCacheGet() {
  return invoke("files_cache_get");
}

/** Removes those copies; each is fetched again when next shown. */
export function filesCacheDelete() {
  return invoke("files_cache_delete");
}

/** Whether this device copies an image to the clipboard. */
export function filesCopySupported() {
  return invoke("files_copy_supported");
}

/**
 * Copies the image to the clipboard.
 * @param {string} id
 */
export function filesCopy(id) {
  return invoke("files_copy", { id });
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

/** One community, `{community, membership}`. @param {string} id */
export function communitiesGet(id) {
  return invoke("communities_get", { id });
}

/** @param {string} name */
export function communitiesCreate(name) {
  return invokeForm("communities_create", { name });
}

/**
 * @param {string} id
 * @param {string} name
 */
export function communitiesUpdate(id, name) {
  return invokeForm("communities_update", { id, name });
}

/** @param {string} id */
export function communitiesDelete(id) {
  return invoke("communities_delete", { id });
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

// The quick picker, the desktop app's second window (../../quick-picker/).

/**
 * The signed-in person's posts and images, newest first, each
 * `{kind, id, label, size}`.
 */
export function quickPickerList() {
  return invoke("quick_picker_list");
}

/**
 * Copies the item, closes the picker and pastes where the person was typing.
 * @param {string} kind "text" or "image"
 * @param {string} id
 */
export function quickPickerPaste(kind, id) {
  return invoke("quick_picker_paste", { kind, id });
}

export function quickPickerClose() {
  return invoke("quick_picker_close");
}

/** Closes the picker and brings the app's window to the front, where a sign-in starts. */
export function quickPickerOpenApp() {
  return invoke("quick_picker_open_app");
}

/**
 * Asks macOS for the Accessibility access pasting needs.
 * @returns {Promise<boolean>} whether it is granted now
 */
export function quickPickerAccessibilityRequest() {
  return invoke("quick_picker_accessibility_request");
}

/**
 * Calls back each time the picker opens, with `{accessibility_trusted}`.
 * @param {(opened: {accessibility_trusted: boolean}) => void} callback
 * @returns {() => void} stops listening
 */
export function onQuickPickerOpened(callback) {
  const stopping = listen("quick-picker-opened", (event) => callback(event.payload));
  return () => {
    stopping.then((stop) => stop());
  };
}
