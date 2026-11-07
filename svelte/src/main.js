import { mount } from "svelte";
import { Router } from "sv-router";
import { isTauri } from "@tauri-apps/api/core";
// Registers the pages with the router before it mounts.
import "./lib/router.js";

// Where the pages run, for the styles that keep clear of the window chrome
// (styles.css): "desktop" under the Tauri window's title bar, "mobile"
// under a phone's status bar, "web" in a browser.
document.documentElement.dataset.app = isTauri() ? (navigator.maxTouchPoints > 0 ? "mobile" : "desktop") : "web";

mount(Router, { target: document.getElementById("app") });
