// app_core over the web server's JSON API (../../../../axum/src/). An
// error answer, `{"error": "..."}`, is thrown with its message.

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
  if (!response.ok) throw new Error(await errorMessage(response));
  if (response.status === 204) return undefined;
  return response.json();
}

/** @param {Response} response */
async function errorMessage(response) {
  try {
    const { error } = await response.json();
    if (typeof error === "string") return error;
  } catch {
    // Not the API's JSON: fall back to the status
  }
  return `${response.status} ${response.statusText}`;
}

/** @param {string} id */
const postPath = (id) => `/api/posts/${encodeURIComponent(id)}`;

/** The user signed in, whom the server takes the request to be from; an error when nobody is. */
export function usersMe() {
  return send("GET", "/api/users/me");
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
export function conflictsLetGo(_id) {
  return Promise.reject(new Error("nothing to let go on the web"));
}
