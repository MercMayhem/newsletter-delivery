-- Your SQL goes here
UPDATE subscriptions
    SET status = 'confirmed'
    WHERE status IS NULL;
ALTER TABLE subscriptions ALTER COLUMN status SET NOT NULL;
