// app_core over the Tauri app's commands (../../../../tauri/src/).
import { invoke } from "@tauri-apps/api/core";

export function postsList() {
  return invoke("posts_list");
}

/** @param {string} id */
export function postsGet(id) {
  return invoke("posts_get", { id });
}

/** @param {string} body */
export function postsCreate(body) {
  return invoke("posts_create", { body });
}

/**
 * @param {string} id
 * @param {string} body
 */
export function postsUpdate(id, body) {
  return invoke("posts_update", { id, body });
}

/** @param {string} id */
export function postsDelete(id) {
  return invoke("posts_delete", { id });
}
