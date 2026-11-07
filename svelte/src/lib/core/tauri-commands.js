// app_core over the Tauri app's commands (../../../../tauri/src/).
import { invoke } from "@tauri-apps/api/core";

export function listPosts() {
  return invoke("list_posts");
}

/** @param {string} id */
export function getPost(id) {
  return invoke("get_post", { id });
}
