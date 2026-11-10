// app_core over the web server's JSON API (../../../../axum/src/). An
// error answer, `{"error": "..."}`, is thrown with its message, as a
// FormError with the fields' messages when it has `"errors"`.
import { FormError } from "./form_error.js";
import { putFile } from "./uploads.js";

/**
 * @param {string} method
 * @param {string} path
 * @param {unknown} [body] sent as JSON
 */
async function send(method, path, body) {
  const response = await fetch(path, {
    method,
    headers: { Accept: "application/json", ...(body === undefined ? {} : { "Content-Type": "application/json" }) },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  if (!response.ok) throw await errorOf(response);
  if (response.status === 204) return undefined;
  return response.json();
}

/** @param {Response} response */
async function errorOf(response) {
  try {
    const { error, errors } = await response.json();
    if (errors) return new FormError(error, errors);
    if (typeof error === "string") return new Error(error);
  } catch {
    // Not the API's JSON: fall back to the status
  }
  return new Error(`${response.status} ${response.statusText}`);
}

/** @param {string} id */
const postPath = (id) => `/api/posts/${encodeURIComponent(id)}`;

/** The user signed in, whom the server takes the request to be from; an error when nobody is. */
export function usersMe() {
  return send("GET", "/api/users/me");
}

/**
 * Where to PUT the picture the person picked, once the server takes it.
 * @param {string} contentType
 * @param {number} size in bytes
 * @returns {Promise<{upload_url: string, content_type: string, key: string}>}
 */
export function avatarsCreateUpload(contentType, size) {
  return send("POST", "/api/avatars/uploads", { content_type: contentType, size });
}

/**
 * Makes the uploaded file the user's picture; answers the user.
 * @param {string} key
 */
export function avatarsUpdate(key) {
  return send("PUT", "/api/avatars", { key });
}

// A sign-up or a sign-in emails a code and answers `{pending}`; the code
// answers the user and sets the cookie the browser sends from then on, or,
// with a device's challenge, `{code}` for the device and no cookie. A
// sign-out clears the cookie.

/**
 * @param {string} email
 * @param {string} password
 * @returns {Promise<{pending: string}>}
 */
export function accountsSignUp(email, password) {
  return send("POST", "/api/accounts/sign-up", { email, password });
}

/**
 * @param {string} email
 * @param {string} password
 * @returns {Promise<{pending: string}>}
 */
export function accountsSignIn(email, password) {
  return send("POST", "/api/accounts/sign-in", { email, password });
}

/**
 * @param {string} pending
 * @param {string} code the six digits emailed
 * @param {string} [challenge] the device's, when it opened this page
 * @returns {Promise<{user: {id: string, email: string}, token: string} | {code: string}>}
 */
export function accountsCode(pending, code, challenge) {
  return send("POST", "/api/accounts/code", { pending, code, challenge: challenge || null });
}

/**
 * A new code emailed for the pending sign-in; the old one stops working.
 * @param {string} pending
 */
export function accountsCodeResend(pending) {
  return send("POST", "/api/accounts/code/resend", { pending });
}

// The browser is where a sign-in happens; nothing waits on it.

/** @param {string} [_email] */
export function accountsSignInStart(_email) {
  return Promise.reject(new Error("the web signs in on its own page"));
}

export function accountsSignInPending() {
  return Promise.resolve(null);
}

/** @param {string} _code */
export function accountsSignInFinish(_code) {
  return Promise.reject(new Error("the web signs in on its own page"));
}

export function accountsSignInCancel() {
  return Promise.resolve(null);
}

export function accountsSignOut() {
  return send("DELETE", "/api/accounts/sign-out");
}

// The web is one browser session: sign in and sign out, no accounts kept
// on the device. The accounts page is never linked.

export const hasDeviceAccounts = false;

export function deviceAccountsList() {
  return Promise.resolve([]);
}

/** @param {string} _userId */
export function deviceAccountsSwitch(_userId) {
  return Promise.reject(new Error("no accounts are kept on the web"));
}

export function deviceAccountsDeactivate() {
  return Promise.resolve(null);
}

/** @param {string} _userId @param {boolean} _losingChanges */
export function deviceAccountsRemove(_userId, _losingChanges) {
  return Promise.reject(new Error("no accounts are kept on the web"));
}

export function postsList() {
  return send("GET", "/api/posts");
}

/** @param {string} id */
export function postsGet(id) {
  return send("GET", postPath(id));
}

/**
 * @param {string} body
 * @param {string[]} communityIds the communities the post is in
 */
export function postsCreate(body, communityIds) {
  return send("POST", "/api/posts", { body, community_ids: communityIds });
}

/**
 * @param {string} id
 * @param {string} body
 * @param {string[]} communityIds the communities the post is in, exactly
 */
export function postsUpdate(id, body, communityIds) {
  return send("PUT", postPath(id), { body, community_ids: communityIds });
}

/** @param {string} id */
export function postsDelete(id) {
  return send("DELETE", postPath(id));
}

/** @param {string} id */
const filePath = (id) => `/api/files/${encodeURIComponent(id)}`;

/** The user's files, each `{file, shown_as, state, error}`. */
export function filesList() {
  return send("GET", "/api/files");
}

/** @param {string} id */
export function filesGet(id) {
  return send("GET", filePath(id));
}

/**
 * Adds the file the person picked: its bytes go straight to object
 * storage, then the server writes its row; answers the file.
 * @param {File} file
 * @param {string} name
 * @param {(fraction: number) => void} [onProgress] from 0 to 1
 */
export async function filesCreate(file, name, onProgress) {
  const upload = await send("POST", "/api/files/upload", { content_type: file.type, size: file.size });
  if (!upload.uploaded) await putFile(upload, file, onProgress);
  return send("POST", "/api/files", { id: upload.id, name, content_type: upload.content_type, size: file.size });
}

/**
 * @param {string} id
 * @param {string} name
 */
export function filesUpdate(id, name) {
  return send("PUT", filePath(id), { name });
}

/** @param {string} id */
export function filesDelete(id) {
  return send("DELETE", filePath(id));
}

/**
 * Where a page reads the file's bytes from, for an `<img>` or a frame.
 * @param {string} id
 */
export function filesContentUrl(id) {
  return `${filePath(id)}/content`;
}

export function filesDownloadSupported() {
  return Promise.resolve(true);
}

/**
 * Has the browser save the file under its name.
 * @param {string} id
 * @returns {Promise<string | null>} where it was saved, when known
 */
export function filesDownload(id) {
  window.location.assign(`${filePath(id)}/content?download=1`);
  return Promise.resolve(null);
}

/** Every community, each with the user's membership when they are in it. */
export function communitiesList() {
  return send("GET", "/api/communities");
}

/** @param {string} name */
export function communitiesCreate(name) {
  return send("POST", "/api/communities", { name });
}

/** @param {string} communityId */
export function membershipsJoin(communityId) {
  return send("PUT", `/api/memberships/${encodeURIComponent(communityId)}`);
}

/** @param {string} communityId */
export function membershipsLeave(communityId) {
  return send("DELETE", `/api/memberships/${encodeURIComponent(communityId)}`);
}

// The web has nothing to sync: it is the server. The nav shows no sync
// status, and the conflicts page shows none.

export function syncRun() {
  return Promise.resolve(null);
}

export function syncStatus() {
  return Promise.resolve(null);
}

/** @param {boolean} _offline */
export function syncSetOffline(_offline) {
  return Promise.resolve(null);
}

/**
 * @param {(outcome: unknown) => void} _callback
 * @returns {() => void} stops listening
 */
export function onSynced(_callback) {
  return () => {};
}

export function conflictsList() {
  return Promise.resolve([]);
}

export function pendingChangesList() {
  return Promise.resolve([]);
}

/** @param {string} _id */
export function conflictsReapply(_id) {
  return Promise.reject(new Error("nothing to reapply on the web"));
}

/** @param {string} _id */
export function conflictsTryAgain(_id) {
  return Promise.reject(new Error("nothing to try again on the web"));
}

/** @param {string} _id */
export function conflictsLetGo(_id) {
  return Promise.reject(new Error("nothing to let go on the web"));
}
