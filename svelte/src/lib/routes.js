// Every page's address, so pages link to each other through here and never
// spell out a path themselves. The paths are the router's too (router.js),
// so they are named once, here. A page takes what it shows from its path,
// the way a web server would read it, so a page is a plain link, from
// another page or from a bookmark.

export const HOME_PATH = "/";
export const ACCOUNTS_INDEX_PATH = "/accounts";
export const ACCOUNTS_SIGN_UP_PATH = "/accounts/sign-up";
export const ACCOUNTS_SIGN_IN_PATH = "/accounts/sign-in";
export const POSTS_INDEX_PATH = "/posts";
export const POSTS_NEW_PATH = "/posts/new";
export const POSTS_SHOW_PATH = "/posts/:id";
export const POSTS_EDIT_PATH = "/posts/:id/edit";
export const COMMUNITIES_INDEX_PATH = "/communities";
export const CONFLICTS_INDEX_PATH = "/conflicts";

/** The pages anyone sees; every other page needs someone signed in. */
export const OPEN_PATHS = [HOME_PATH, ACCOUNTS_INDEX_PATH, ACCOUNTS_SIGN_UP_PATH, ACCOUNTS_SIGN_IN_PATH];

export function homeUrl() {
  return HOME_PATH;
}

/** The accounts on this device, inside the app only. */
export function accountsIndexUrl() {
  return ACCOUNTS_INDEX_PATH;
}

/** @param {string} [challenge] a device's, carried through from the sign-in page it opened */
export function accountsSignUpUrl(challenge) {
  return withQuery(ACCOUNTS_SIGN_UP_PATH, { challenge });
}

/**
 * @param {string} [email] filled in on the page, for an account signed out on this device
 * @param {string} [challenge] a device's, when it opened the page in the browser to sign in
 */
export function accountsSignInUrl(email, challenge) {
  return withQuery(ACCOUNTS_SIGN_IN_PATH, { email, challenge });
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

export function communitiesIndexUrl() {
  return COMMUNITIES_INDEX_PATH;
}

export function conflictsIndexUrl() {
  return CONFLICTS_INDEX_PATH;
}
