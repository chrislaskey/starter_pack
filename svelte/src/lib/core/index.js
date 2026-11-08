// The pages' calls into app_core (../../../../core). Inside the Tauri app
// they are the app's commands; in a browser they are the web server's JSON
// API. Both answer with the same data, serialized from the same Rust
// types, so a page never knows which it is on. Add a function to both
// files when app_core gains one.
import { isTauri } from "@tauri-apps/api/core";
import * as httpApi from "./http-api.js";
import * as tauriCommands from "./tauri-commands.js";

const core = isTauri() ? tauriCommands : httpApi;

export const usersMe = core.usersMe;
export const accountsSignUp = core.accountsSignUp;
export const accountsSignIn = core.accountsSignIn;
export const accountsSignOut = core.accountsSignOut;
export const hasDeviceAccounts = core.hasDeviceAccounts;
export const deviceAccountsList = core.deviceAccountsList;
export const deviceAccountsSwitch = core.deviceAccountsSwitch;
export const deviceAccountsDeactivate = core.deviceAccountsDeactivate;
export const deviceAccountsRemove = core.deviceAccountsRemove;
export const postsList = core.postsList;
export const postsGet = core.postsGet;
export const postsCreate = core.postsCreate;
export const postsUpdate = core.postsUpdate;
export const postsDelete = core.postsDelete;
export const communitiesList = core.communitiesList;
export const communitiesCreate = core.communitiesCreate;
export const membershipsJoin = core.membershipsJoin;
export const membershipsLeave = core.membershipsLeave;
export const syncRun = core.syncRun;
export const syncStatus = core.syncStatus;
export const syncSetOffline = core.syncSetOffline;
export const onSynced = core.onSynced;
export const conflictsList = core.conflictsList;
export const conflictsReapply = core.conflictsReapply;
export const conflictsLetGo = core.conflictsLetGo;
export const pendingChangesList = core.pendingChangesList;
