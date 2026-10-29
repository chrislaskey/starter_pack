// Every page's address of the main window, so pages link to each other
// through here and never spell out a path themselves. The paths are the
// router's too (router.js), so they are named once, here, and they mirror
// the Phoenix app's where a page has a counterpart (`/communities`). A page
// takes what it shows from its path and query, the way a web server would
// read them, so a page is a plain link, from another page or from a bookmark.

export const HOME_PATH = "/";
export const COMMUNITIES_INDEX_PATH = "/communities";
export const TMUX_EXAMPLE_PATH = "/tmux-example";

export function homeUrl() {
  return HOME_PATH;
}

export function communitiesIndexUrl() {
  return COMMUNITIES_INDEX_PATH;
}

export function tmuxExampleUrl() {
  return TMUX_EXAMPLE_PATH;
}
