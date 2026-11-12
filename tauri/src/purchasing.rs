//! The accounts app's purchasing in the desktop app: each command asks
//! the server's API as the signed-in person, see
//! `app_core::accounts::online`, and answers its JSON as it came. Stripe's
//! pages open in the system browser and come back to a page there that
//! says to return to the app, whose pages ask again when it has the
//! focus. A phone buys through its store instead, not built yet, so it
//! is refused there. See `docs/design/purchasing.md`, step 7.

use serde_json::{Value, json};
use tauri::{AppHandle, State};
use uuid::Uuid;

use app_core::accounts::online;

use crate::device_accounts::DeviceAccounts;
use crate::sync::SyncServer;

#[tauri::command]
pub async fn accounts_products_list(
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
) -> Result<Value, String> {
    let deployment = device.active().await.map_err(|error| error.to_string())?;
    online::products_list(&deployment, server.transport())
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn accounts_products_get(
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
    id: Uuid,
) -> Result<Value, String> {
    let deployment = device.active().await.map_err(|error| error.to_string())?;
    online::products_get(&deployment, server.transport(), id)
        .await
        .map_err(|error| error.to_string())
}

/// A checkout, its Stripe page opened in the browser: answers the
/// checkout with no `url`, as the app has nothing to go to.
#[tauri::command]
pub async fn accounts_checkouts_create(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
    price_id: Uuid,
) -> Result<Value, String> {
    refused_on_a_phone()?;
    let deployment = device.active().await.map_err(|error| error.to_string())?;
    let created = online::checkouts_create(&deployment, server.transport(), price_id)
        .await
        .map_err(|error| error.to_string())?;
    browser_open(&app, created.get("url"))?;
    Ok(json!({ "checkout": created.get("checkout"), "url": null }))
}

/// A checkout; once it is paid, the plan kept on the device is asked
/// for again.
#[tauri::command]
pub async fn accounts_checkouts_get(
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
    id: Uuid,
) -> Result<Value, String> {
    let deployment = device.active().await.map_err(|error| error.to_string())?;
    let checkout = online::checkouts_get(&deployment, server.transport(), id)
        .await
        .map_err(|error| error.to_string())?;
    if checkout.get("status") == Some(&json!("paid")) {
        plan_refresh(&deployment, server.transport()).await;
    }
    Ok(checkout)
}

#[tauri::command]
pub async fn accounts_purchases_list(
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
) -> Result<Value, String> {
    let deployment = device.active().await.map_err(|error| error.to_string())?;
    online::purchases_list(&deployment, server.transport())
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn accounts_purchases_get(
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
    id: Uuid,
) -> Result<Value, String> {
    let deployment = device.active().await.map_err(|error| error.to_string())?;
    online::purchases_get(&deployment, server.transport(), id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn accounts_subscriptions_list(
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
) -> Result<Value, String> {
    let deployment = device.active().await.map_err(|error| error.to_string())?;
    online::subscriptions_list(&deployment, server.transport())
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn accounts_subscriptions_get(
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
    id: Uuid,
) -> Result<Value, String> {
    let deployment = device.active().await.map_err(|error| error.to_string())?;
    online::subscriptions_get(&deployment, server.transport(), id)
        .await
        .map_err(|error| error.to_string())
}

/// A change to a plan, then the plan kept on the device asked for again.
#[tauri::command]
pub async fn accounts_subscriptions_update(
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
    id: Uuid,
    change: Value,
) -> Result<Value, String> {
    refused_on_a_phone()?;
    let deployment = device.active().await.map_err(|error| error.to_string())?;
    let changed = online::subscriptions_update(&deployment, server.transport(), id, &change)
        .await
        .map_err(|error| error.to_string())?;
    plan_refresh(&deployment, server.transport()).await;
    Ok(changed)
}

/// Stripe's Customer Portal, opened in the browser: answers no `url`.
#[tauri::command]
pub async fn accounts_subscriptions_portal_create(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
    id: Uuid,
) -> Result<Value, String> {
    refused_on_a_phone()?;
    let deployment = device.active().await.map_err(|error| error.to_string())?;
    let portal = online::subscriptions_portal_create(&deployment, server.transport(), id)
        .await
        .map_err(|error| error.to_string())?;
    browser_open(&app, portal.get("url"))?;
    Ok(json!({ "url": null }))
}

/// Asks for the plan again, after a change; a failure leaves the plan
/// kept as it was, for the next sync to try.
pub async fn plan_refresh(
    deployment: &app_core::deployment::Deployment,
    server: &app_core::sync::Http,
) {
    if let Err(error) = online::plan_refresh(deployment, server).await {
        log::warn!("could not refresh the plan: {error}");
    }
}

fn browser_open(app: &AppHandle, url: Option<&Value>) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let url = url
        .and_then(Value::as_str)
        .ok_or("The server answered no page to open")?;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|error| error.to_string())
}

/// A phone sells through its store's in-app purchase, see
/// `docs/design/purchasing.md`, step 8.
fn refused_on_a_phone() -> Result<(), String> {
    if cfg!(mobile) {
        return Err("On a phone, purchases are not available yet".to_string());
    }
    Ok(())
}
