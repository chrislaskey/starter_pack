// Every page's address, so pages link to each other through here and never
// spell out a path themselves. The paths are the router's too (router.js),
// so they are named once, here. A page takes what it shows from its path,
// the way a web server would read it, so a page is a plain link, from
// another page or from a bookmark.

export const HOME_PATH = "/";
export const POSTS_INDEX_PATH = "/posts";
export const POST_SHOW_PATH = "/posts/:id";

export function homeUrl() {
  return HOME_PATH;
}

export function postsIndexUrl() {
  return POSTS_INDEX_PATH;
}

/** @param {string} id */
export function postShowUrl(id) {
  return POST_SHOW_PATH.replace(":id", encodeURIComponent(id));
}
