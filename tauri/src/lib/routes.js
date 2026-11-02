// Every page's address of the main window, so pages link to each other
// through here and never spell out a path themselves. The paths are the
// router's too (router.js), so they are named once, here, and they mirror
// the Phoenix app's where a page has a counterpart (`/communities`). A page
// takes what it shows from its path and query, the way a web server would
// read them, so a page is a plain link, from another page or from a bookmark.

export const HOME_PATH = "/";
export const COMMUNITIES_INDEX_PATH = "/communities";
export const COMMUNITY_SHOW_PATH = "/communities/:id";
export const POSTS_INDEX_PATH = "/posts";
export const TAGS_INDEX_PATH = "/tags";
export const PROFILE_PATH = "/profile";
export const TMUX_EXAMPLE_PATH = "/tmux-example";

export function homeUrl() {
  return HOME_PATH;
}

export function communitiesIndexUrl() {
  return COMMUNITIES_INDEX_PATH;
}

/** @param {string} id */
export function communityShowUrl(id) {
  return COMMUNITY_SHOW_PATH.replace(":id", encodeURIComponent(id));
}

export function postsIndexUrl() {
  return POSTS_INDEX_PATH;
}

export function tagsIndexUrl() {
  return TAGS_INDEX_PATH;
}

export function profileUrl() {
  return PROFILE_PATH;
}

export function tmuxExampleUrl() {
  return TMUX_EXAMPLE_PATH;
}
