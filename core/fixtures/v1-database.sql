-- Base DmxMoney 1.0.22 synthétique : schéma produit par src-tauri/src/db.rs (1.x) et données
-- représentatives (virement lié, échéance déjà générée, préférences sérialisées en JSON).

CREATE TABLE accounts (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    "type" TEXT NOT NULL,
    "initialBalance" REAL NOT NULL,
    color TEXT,
    icon TEXT
);

CREATE TABLE transactions (
    id TEXT PRIMARY KEY,
    date TEXT NOT NULL,
    "accountId" TEXT NOT NULL,
    "type" TEXT NOT NULL,
    amount REAL NOT NULL,
    category TEXT NOT NULL,
    description TEXT,
    checked BOOLEAN DEFAULT 0,
    "isTransfer" BOOLEAN DEFAULT 0,
    "linkedTransactionId" TEXT,
    FOREIGN KEY("accountId") REFERENCES accounts(id)
);

CREATE TABLE categories (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    icon TEXT NOT NULL,
    color TEXT NOT NULL
);

CREATE TABLE scheduled_transactions (
    id TEXT PRIMARY KEY,
    description TEXT NOT NULL,
    amount REAL NOT NULL,
    "type" TEXT NOT NULL,
    frequency TEXT NOT NULL,
    "accountId" TEXT NOT NULL,
    "nextDate" TEXT NOT NULL,
    category TEXT NOT NULL,
    "toAccountId" TEXT,
    "includeInForecast" BOOLEAN DEFAULT 1,
    "endDate" TEXT,
    "budgetId" TEXT,
    FOREIGN KEY("accountId") REFERENCES accounts(id)
);

CREATE TABLE budgets (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    amount REAL NOT NULL,
    category TEXT NOT NULL,
    "accountId" TEXT,
    FOREIGN KEY("accountId") REFERENCES accounts(id)
);

CREATE TABLE settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    theme TEXT NOT NULL DEFAULT 'system',
    "primaryColor" TEXT NOT NULL DEFAULT '#6366f1',
    "displayStyle" TEXT NOT NULL DEFAULT 'modern',
    "windowPositionX" INTEGER,
    "windowPositionY" INTEGER,
    "windowSizeWidth" INTEGER,
    "windowSizeHeight" INTEGER,
    "componentSpacing" INTEGER NOT NULL DEFAULT 6,
    "componentPadding" INTEGER NOT NULL DEFAULT 6,
    "accountGroups" TEXT,
    "customGroups" TEXT,
    "customGroupsOrder" TEXT,
    "accountsOrder" TEXT,
    "lastSeenVersion" TEXT,
    "mobileAccessEnabled" BOOLEAN NOT NULL DEFAULT 0,
    "mobileAccessToken" TEXT,
    "mobileAccessPort" INTEGER NOT NULL DEFAULT 8799,
    "dismissedBudgetSuggestions" TEXT,
    "dismissedScheduledSuggestions" TEXT,
    "predictionTimeRange" TEXT NOT NULL DEFAULT 'year',
    "predictionCustomEndDate" TEXT,
    "predictionAlertThreshold" REAL NOT NULL DEFAULT 0,
    "predictionMonthStartsOnFirst" BOOLEAN NOT NULL DEFAULT 1,
    "predictionFakeTransactions" TEXT,
    "analyticsTimeRange" TEXT NOT NULL DEFAULT 'year',
    "analyticsCustomStartDate" TEXT,
    "analyticsCustomEndDate" TEXT,
    "analyticsMonthStartsOnFirst" BOOLEAN NOT NULL DEFAULT 1,
    "analyticsHiddenExpenseCategories" TEXT,
    "analyticsHiddenIncomeCategories" TEXT,
    "scheduledDueRange" TEXT NOT NULL DEFAULT 'all',
    "settingsRevision" INTEGER NOT NULL DEFAULT 0,
    "settingsFieldVersions" TEXT NOT NULL DEFAULT '{}',
    "secureBridgeEnabled" BOOLEAN NOT NULL DEFAULT 0,
    "secureBridgeDomain" TEXT,
    "secureBridgeAppUrl" TEXT,
    "secureBridgeLocalHost" TEXT,
    "secureBridgeDeviceId" TEXT,
    "secureBridgeCertificateExpiresAt" TEXT,
    "secureBridgeDnsRecordId" TEXT,
    "secureBridgeDnsLastUpdatedAt" TEXT,
    "secureBridgeLastError" TEXT,
    "secureBridgeManagedServiceUrl" TEXT,
    "secureBridgeManagedRegisteredAt" TEXT,
    "secureBridgeManagedDeviceSecret" TEXT
);

CREATE INDEX idx_transactions_account_id ON transactions("accountId");
CREATE INDEX idx_transactions_date ON transactions(date);
CREATE INDEX idx_scheduled_account_id ON scheduled_transactions("accountId");
CREATE INDEX idx_budgets_account_id ON budgets("accountId");
CREATE INDEX idx_budgets_category ON budgets(category);

CREATE TABLE mobile_passkeys (
    id TEXT PRIMARY KEY,
    credential_id TEXT NOT NULL UNIQUE,
    public_key TEXT NOT NULL,
    counter INTEGER NOT NULL DEFAULT 0,
    device_label TEXT,
    created_at TEXT NOT NULL,
    last_used_at TEXT,
    revoked_at TEXT
);

CREATE TABLE mobile_pairing_tokens (
    id TEXT PRIMARY KEY,
    token_hash TEXT NOT NULL UNIQUE,
    expires_at TEXT NOT NULL,
    consumed_at TEXT,
    created_at TEXT NOT NULL
);

CREATE TABLE mobile_sessions (
    id TEXT PRIMARY KEY,
    session_hash TEXT NOT NULL UNIQUE,
    csrf_hash TEXT NOT NULL,
    passkey_id TEXT,
    device_label TEXT,
    expires_at TEXT NOT NULL,
    created_at TEXT NOT NULL,
    last_used_at TEXT,
    revoked_at TEXT
);

CREATE TABLE mobile_auth_challenges (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    state_json TEXT NOT NULL,
    session_id TEXT,
    device_label TEXT,
    expires_at TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE sync_state (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    version INTEGER NOT NULL DEFAULT 0
);
INSERT INTO sync_state (id, version) VALUES (1, 0);

CREATE TRIGGER trg_sync_transactions_INSERT AFTER INSERT ON transactions
BEGIN
    UPDATE sync_state SET version = version + 1 WHERE id = 1;
END;

INSERT INTO accounts VALUES ('acc-courant', 'Compte courant', 'Courant', 1500.0, '#3b82f6', 'Wallet');
INSERT INTO accounts VALUES ('acc-epargne', 'Livret A', 'Épargne', 5000.0, '#10b981', 'PiggyBank');

INSERT INTO categories VALUES ('1', 'Loyer / Prêt', 'Home', '#1e3a8a');
INSERT INTO categories VALUES ('5', 'Alimentation', 'ShoppingBag', '#ef4444');
INSERT INTO categories VALUES ('15', 'Loisirs / Cinéma', 'Gamepad2', '#8b5cf6');
INSERT INTO categories VALUES ('21', 'Salaire', 'Banknote', '#16a34a');
INSERT INTO categories VALUES ('23', 'Cadeaux reçus', 'Gift', '#db2777');
INSERT INTO categories VALUES ('transfer', 'Virement', 'ArrowRightLeft', '#6366f1');

INSERT INTO transactions (id, date, "accountId", "type", amount, category, description, checked, "isTransfer", "linkedTransactionId") VALUES
    ('t-salaire', '2026-09-01', 'acc-courant', 'income', 2500.0, '21', 'Salaire septembre', 1, 0, NULL),
    ('scheduled:sch-loyer:2026-09-01:single', '2026-09-01', 'acc-courant', 'expense', 850.0, '1', 'Loyer', 0, 0, NULL),
    ('t-courses', '2026-09-03', 'acc-courant', 'expense', 84.2, '5', 'Carrefour', 1, 0, NULL),
    ('tr-from', '2026-09-05', 'acc-courant', 'expense', 200.0, 'transfer', 'Épargne mensuelle', 0, 1, 'tr-to'),
    ('tr-to', '2026-09-05', 'acc-epargne', 'income', 200.0, 'transfer', 'Épargne mensuelle', 0, 1, 'tr-from');

-- Le loyer du 1er septembre a déjà été généré, mais l'échéance n'a pas encore avancé.
INSERT INTO scheduled_transactions (id, description, amount, "type", frequency, "accountId", "nextDate", category, "toAccountId", "includeInForecast", "budgetId", "endDate") VALUES
    ('sch-loyer', 'Loyer', 850.0, 'expense', 'monthly', 'acc-courant', '2026-09-01', '1', NULL, 0, NULL, NULL),
    ('sch-courses', 'Courses hebdo', 120.0, 'expense', 'weekly', 'acc-courant', '2026-09-19', '5', NULL, 1, NULL, NULL);

INSERT INTO budgets VALUES ('bud-loisirs', 'Loisirs', 150.0, '15', NULL);

INSERT INTO settings (
    id, theme, "primaryColor", "accountGroups", "customGroups", "lastSeenVersion",
    "dismissedBudgetSuggestions", "predictionAlertThreshold", "predictionFakeTransactions",
    "analyticsHiddenExpenseCategories", "settingsRevision", "settingsFieldVersions",
    "secureBridgeEnabled", "secureBridgeDeviceId"
) VALUES (
    1, 'dark', '#AF52DE', '{"acc-epargne":"Épargne"}', '["Épargne"]', '1.0.22',
    '["9|all"]', 100.0,
    '[{"id":"fake-1","date":"2026-12-24","accountId":"acc-courant","type":"expense","amount":300,"category":"23","description":"Cadeaux","enabled":true}]',
    '["transfer"]', 12, '{"theme":12}',
    1, 'dmx-0001'
);
