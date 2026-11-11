-- 0006: billing entries and budgets the user records (ADR-028).
--
-- Amounts are integer billionths of the entry's currency (ADR-004). An entry's kind is
-- its cost kind: subscriptions are subscription spend, charges are actual charges. They
-- are never added to each other or to API-equivalent cost.

CREATE TABLE billing_entries (
    id          INTEGER PRIMARY KEY,
    kind        TEXT NOT NULL CHECK (kind IN ('subscription', 'charge')),
    label       TEXT NOT NULL,
    provider_id TEXT,
    amount_nano INTEGER NOT NULL CHECK (amount_nano > 0),
    currency    TEXT NOT NULL CHECK (length(currency) = 3),
    recurrence  TEXT NOT NULL CHECK (recurrence IN ('once', 'monthly', 'yearly')),
    starts_on   TEXT NOT NULL,                     -- YYYY-MM-DD
    ends_on     TEXT,                              -- YYYY-MM-DD, inclusive
    CHECK (ends_on IS NULL OR (recurrence <> 'once' AND ends_on >= starts_on))
) STRICT;

-- A monthly limit on API-equivalent cost, for all usage or for one project.
CREATE TABLE budgets (
    id             INTEGER PRIMARY KEY,
    label          TEXT NOT NULL,
    limit_nano_usd INTEGER NOT NULL CHECK (limit_nano_usd > 0),
    warn_percent   INTEGER NOT NULL CHECK (warn_percent BETWEEN 1 AND 100),
    project_id     INTEGER REFERENCES projects (id)
) STRICT;
