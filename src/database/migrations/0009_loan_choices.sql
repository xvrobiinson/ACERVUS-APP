-- Acervus Finance schema v9 (2026-11-10, version 0.15): each loan's way of
-- charging late fees (installment, monthly, overdue, balance; older loans keep
-- today's way) and what part of its interest is reinvested into capital,
-- from each date ("2026-11-10:60;2026-12-01:0"; empty: all kept as profit).

ALTER TABLE loans ADD COLUMN late_fee_mode TEXT NOT NULL DEFAULT 'installment'
    CHECK (late_fee_mode IN ('installment', 'monthly', 'overdue', 'balance'));
ALTER TABLE loans ADD COLUMN reinvest TEXT NOT NULL DEFAULT '';
