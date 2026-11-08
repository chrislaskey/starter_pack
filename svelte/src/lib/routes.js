// Every page's address, so pages link to each other through here and never
// spell out a path themselves. The paths are the router's too (router.js),
// so they are named once, here. A page takes what it shows from its path,
// the way a web server would read it, so a page is a plain link, from
// another page or from a bookmark.

export const HOME_PATH = "/";
export const POSTS_INDEX_PATH = "/posts";
export const POSTS_NEW_PATH = "/posts/new";
export const POSTS_SHOW_PATH = "/posts/:id";
export const POSTS_EDIT_PATH = "/posts/:id/edit";
export const COMMUNITIES_INDEX_PATH = "/communities";
export const CONFLICTS_INDEX_PATH = "/conflicts";

export function homeUrl() {
  return HOME_PATH;
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
