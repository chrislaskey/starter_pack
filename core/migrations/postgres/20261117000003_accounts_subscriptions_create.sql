-- Plans, Stripe's subscriptions as the server last heard them, see
-- src/accounts/subscriptions.rs. Server only. Stripe keeps the history,
-- so a user's rows go with them.
CREATE TABLE accounts_subscriptions (
  id uuid PRIMARY KEY,
  user_id uuid NOT NULL REFERENCES accounts_users (id) ON DELETE CASCADE,
  product_id uuid NOT NULL REFERENCES accounts_products (id) ON DELETE CASCADE,
  price_id uuid NOT NULL REFERENCES accounts_prices (id) ON DELETE CASCADE,
  store text NOT NULL,
  store_id text NOT NULL,
  status text NOT NULL,
  current_period_end timestamptz,
  cancel_at_period_end boolean NOT NULL,
  ended_at timestamptz,
  inserted_at timestamptz NOT NULL,
  updated_at timestamptz NOT NULL,
  UNIQUE (store, store_id)
);

CREATE INDEX accounts_subscriptions_user_id ON accounts_subscriptions (user_id);
