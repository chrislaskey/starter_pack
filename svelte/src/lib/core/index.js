// The pages' calls into app_core (../../../../core). Inside the Tauri app
// they are the app's commands; in a browser they are the web server's JSON
// API. Both answer with the same data, serialized from the same Rust
// types, so a page never knows which it is on. Add a function to both
// files when app_core gains one.
import { isTauri } from "@tauri-apps/api/core";
import * as httpApi from "./http-api.js";
import * as tauriCommands from "./tauri-commands.js";

const core = isTauri() ? tauriCommands : httpApi;

export const listPosts = core.listPosts;
export const getPost = core.getPost;
