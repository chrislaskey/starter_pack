-- A plan change that waits for the end of the period paid for, see
-- src/accounts/subscriptions.rs: the price it moves to, and when.
ALTER TABLE accounts_subscriptions ADD COLUMN scheduled_price_id uuid REFERENCES accounts_prices (id) ON DELETE SET NULL;
ALTER TABLE accounts_subscriptions ADD COLUMN scheduled_at timestamptz;
