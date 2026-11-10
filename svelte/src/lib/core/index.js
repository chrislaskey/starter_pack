// The pages' calls into app_core (../../../../core). Inside the Tauri app
// they are the app's commands; in a browser they are the web server's JSON
// API. Both answer with the same data, serialized from the same Rust
// types, so a page never knows which it is on. Add a function to both
// files when app_core gains one.
import { isTauri } from "@tauri-apps/api/core";
import * as httpApi from "./http-api.js";
import * as tauriCommands from "./tauri-commands.js";
import { putFile } from "./uploads.js";

export { FormError } from "./form_error.js";

const core = isTauri() ? tauriCommands : httpApi;

export const currentUserGet = core.currentUserGet;
export const currentCommunityGet = core.currentCommunityGet;
export const currentCommunityUpdate = core.currentCommunityUpdate;
export const avatarsCreateUpload = core.avatarsCreateUpload;
export const avatarsUpdate = core.avatarsUpdate;
export const accountsSignUp = core.accountsSignUp;
export const accountsSignIn = core.accountsSignIn;
export const accountsCode = core.accountsCode;
export const accountsCodeResend = core.accountsCodeResend;
export const accountsSignInStart = core.accountsSignInStart;
export const accountsSignInPending = core.accountsSignInPending;
export const accountsSignInFinish = core.accountsSignInFinish;
export const accountsSignInCancel = core.accountsSignInCancel;
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
export const filesList = core.filesList;
export const filesGet = core.filesGet;
export const filesCreate = core.filesCreate;
export const filesUpdate = core.filesUpdate;
export const filesDelete = core.filesDelete;
export const filesContentUrl = core.filesContentUrl;
export const filesCacheGet = core.filesCacheGet;
export const filesCacheDelete = core.filesCacheDelete;
export const filesCopySupported = core.filesCopySupported;
export const filesCopy = core.filesCopy;
export const filesDownloadSupported = core.filesDownloadSupported;
export const filesDownload = core.filesDownload;
export const communitiesList = core.communitiesList;
export const communitiesGet = core.communitiesGet;
export const communitiesCreate = core.communitiesCreate;
export const communitiesUpdate = core.communitiesUpdate;
export const communitiesDelete = core.communitiesDelete;
export const membershipsJoin = core.membershipsJoin;
export const membershipsLeave = core.membershipsLeave;
export const syncRun = core.syncRun;
export const syncStatus = core.syncStatus;
export const syncSetOffline = core.syncSetOffline;
export const onSynced = core.onSynced;
export const conflictsList = core.conflictsList;
export const conflictsReapply = core.conflictsReapply;
export const conflictsTryAgain = core.conflictsTryAgain;
export const conflictsLetGo = core.conflictsLetGo;
export const pendingChangesList = core.pendingChangesList;
export const quickPickerList = core.quickPickerList;
export const quickPickerPaste = core.quickPickerPaste;
export const quickPickerClose = core.quickPickerClose;
export const quickPickerOpenApp = core.quickPickerOpenApp;
export const quickPickerAccessibilityRequest = core.quickPickerAccessibilityRequest;
export const onQuickPickerOpened = core.onQuickPickerOpened;

/**
 * Uploads the picture straight to object storage, then makes it the
 * user's picture; answers the user. Throws a FormError for a file the
 * server does not take.
 * @param {File} file
 * @param {(fraction: number) => void} [onProgress] from 0 to 1
 */
export async function avatarsUpload(file, onProgress) {
  const upload = await core.avatarsCreateUpload(file.type, file.size);
  await putFile(upload, file, onProgress);
  return core.avatarsUpdate(upload.key);
}
