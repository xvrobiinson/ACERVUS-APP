-- Acervus Finance schema v8: how a payment was made (cash, transfer, card,
-- check, other), printed on its receipt. Payments from before stay empty.

ALTER TABLE payments ADD COLUMN paid_with TEXT
    CHECK (paid_with IS NULL OR paid_with IN ('cash', 'transfer', 'card', 'check', 'other'));
