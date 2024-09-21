-- This file should undo anything in `up.sql`
ALTER TABLE issue_delivery_queue DROP COLUMN state;
