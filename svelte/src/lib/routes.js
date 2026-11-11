// Every page's address, so pages link to each other through here and never
// spell out a path themselves. The paths are the router's too (router.js),
// so they are named once, here. A page takes what it shows from its path,
// the way a web server would read it, so a page is a plain link, from
// another page or from a bookmark.

// The public website's home page on the web (src/public); the app has no
// page there, so its router sends `/` to the app's home.
export const PUBLIC_HOME_PATH = "/";
export const HOME_PATH = "/home";
export const ACCOUNTS_INDEX_PATH = "/accounts";
export const ACCOUNTS_SIGN_UP_PATH = "/accounts/sign-up";
export const ACCOUNTS_SIGN_IN_PATH = "/accounts/sign-in";
export const POSTS_INDEX_PATH = "/posts";
export const POSTS_NEW_PATH = "/posts/new";
export const POSTS_SHOW_PATH = "/posts/:id";
export const POSTS_EDIT_PATH = "/posts/:id/edit";
export const FILES_INDEX_PATH = "/files";
export const FILES_NEW_PATH = "/files/new";
export const FILES_SHOW_PATH = "/files/:id";
export const FILES_EDIT_PATH = "/files/:id/edit";
export const COMMUNITIES_INDEX_PATH = "/communities";
export const COMMUNITIES_NEW_PATH = "/communities/new";
export const COMMUNITIES_SHOW_PATH = "/communities/:id";
export const COMMUNITIES_EDIT_PATH = "/communities/:id/edit";
export const CONFLICTS_INDEX_PATH = "/conflicts";
export const PROFILE_EDIT_PATH = "/profile";

/** The pages anyone sees; every other page needs someone signed in. */
export const OPEN_PATHS = [HOME_PATH, ACCOUNTS_INDEX_PATH, ACCOUNTS_SIGN_UP_PATH, ACCOUNTS_SIGN_IN_PATH];

export function homeUrl() {
  return HOME_PATH;
}

/** The accounts on this device, inside the app only. */
export function accountsIndexUrl() {
  return ACCOUNTS_INDEX_PATH;
}

// The account pages carry two things in their query, which every link
// between them keeps unless the caller says otherwise: a device's
// challenge, when it opened the page in the browser to sign in, and the
// email to fill in. So the nav's Sign in and Sign up, and the pages'
// own links, all stay a device's sign-in.

/**
 * @param {string} [email] to fill in, else the current address's
 * @param {string} [challenge] a device's, else the current address's
 */
export function accountsSignUpUrl(email = currentQuery("email"), challenge = currentQuery("challenge")) {
  return withQuery(ACCOUNTS_SIGN_UP_PATH, { email, challenge });
}

/**
 * @param {string} [email] to fill in, for an account signed out on this device, else the current address's
 * @param {string} [challenge] a device's, else the current address's
 */
export function accountsSignInUrl(email = currentQuery("email"), challenge = currentQuery("challenge")) {
  return withQuery(ACCOUNTS_SIGN_IN_PATH, { email, challenge });
}

/**
 * A value in the current address's query, or "" when absent or when
 * there is no window yet.
 * @param {string} name
 */
function currentQuery(name) {
  if (typeof window === "undefined") return "";
  return new URLSearchParams(window.location.search).get(name) ?? "";
}

/**
 * @param {string} path
 * @param {Record<string, string | undefined>} values the query, without the empty ones
 */
function withQuery(path, values) {
  const query = new URLSearchParams();
  for (const [name, value] of Object.entries(values)) {
    if (value) query.set(name, value);
  }
  const string = query.toString();
  return string ? `${path}?${string}` : path;
}

export function postsIndexUrl() {
  return POSTS_INDEX_PATH;
}

export function postsNewUrl() {
  return POSTS_NEW_PATH;
}

/** @param {string} id */
export function postsShowUrl(id) {
  return POSTS_SHOW_PATH.replace(":id", encodeURIComponent(id));
}

/** @param {string} id */
export function postsEditUrl(id) {
  return POSTS_EDIT_PATH.replace(":id", encodeURIComponent(id));
}

export function filesIndexUrl() {
  return FILES_INDEX_PATH;
}

export function filesNewUrl() {
  return FILES_NEW_PATH;
}

/** @param {string} id */
export function filesShowUrl(id) {
  return FILES_SHOW_PATH.replace(":id", encodeURIComponent(id));
}

/** @param {string} id */
export function filesEditUrl(id) {
  return FILES_EDIT_PATH.replace(":id", encodeURIComponent(id));
}

export function communitiesIndexUrl() {
  return COMMUNITIES_INDEX_PATH;
}

export function communitiesNewUrl() {
  return COMMUNITIES_NEW_PATH;
}

/** @param {string} id */
export function communitiesShowUrl(id) {
  return COMMUNITIES_SHOW_PATH.replace(":id", encodeURIComponent(id));
}

/** @param {string} id */
export function communitiesEditUrl(id) {
  return COMMUNITIES_EDIT_PATH.replace(":id", encodeURIComponent(id));
}

export function conflictsIndexUrl() {
  return CONFLICTS_INDEX_PATH;
}

export function profileEditUrl() {
  return PROFILE_EDIT_PATH;
}
