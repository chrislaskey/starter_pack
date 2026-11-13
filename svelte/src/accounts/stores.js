// Where a plan or product was bought, as the pages name it. The pages
// read `store` from what the server and the commands answer, never the
// platform, see docs/design/purchasing-phones.md.

/** @typedef {"stripe" | "app_store" | "play_store"} Store */

/** @type {Record<Store, string>} */
const NAMES = { stripe: "the web", app_store: "the App Store", play_store: "Google Play" };

/** @type {Record<Store, string>} */
const DEVICES = { stripe: "on the web", app_store: "on your iPhone", play_store: "on your Android device" };

/**
 * `the App Store`
 * @param {Store} store
 */
export function storeName(store) {
  return NAMES[store] ?? NAMES.stripe;
}

/**
 * `in the App Store on your iPhone`, where a plan from another store is managed.
 * @param {Store} store
 */
export function storeManagedWhere(store) {
  return store === "stripe" ? "on the web" : `in ${storeName(store)} ${DEVICES[store]}`;
}

/**
 * A price as the phone's store sells it, `$9.99 / month`, else as the server has it.
 * @param {{store_price?: string | null, store_period?: string | null}} price
 * @param {(price: any) => string} serverFormat
 */
export function storePriceFormat(price, serverFormat) {
  if (!price.store_price) return serverFormat(price);
  return price.store_period ? `${price.store_price} / ${price.store_period}` : price.store_price;
}
