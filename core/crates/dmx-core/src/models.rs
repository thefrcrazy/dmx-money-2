//! Modèles persistés, sérialisés exactement comme dans DmxMoney 1.x (API compagnon, fichiers `.dmx`).

use serde::{Deserialize, Deserializer, Serialize};

pub const TRANSFER_CATEGORY_ID: &str = "transfer";
pub const TRANSFER_CATEGORY_NAME: &str = "Virement";
pub const TRANSFER_CATEGORY_ICON: &str = "ArrowRightLeft";
pub const TRANSFER_COLOR: &str = "#6366f1";
pub const DEFAULT_ACCOUNT_COLOR: &str = "#3b82f6";
pub const DEFAULT_ACCOUNT_ICON: &str = "Wallet";
pub const UNKNOWN_CATEGORY_NAME: &str = "Inconnu";
pub const UNKNOWN_CATEGORY_ICON: &str = "Tag";
pub const UNKNOWN_CATEGORY_COLOR: &str = "#9ca3af";
pub const UNGROUPED_ACCOUNTS_LABEL: &str = "Non groupés";

macro_rules! string_enum {
    ($(#[$meta:meta])* $name:ident, default = $default:ident, { $($variant:ident => $value:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$variant),+];

            pub fn as_str(self) -> &'static str {
                match self {
                    $($name::$variant => $value),+
                }
            }

            /// Les valeurs inconnues retombent sur la valeur par défaut, comme les formulaires 1.x.
            pub fn parse(value: &str) -> Self {
                match value {
                    $($value => $name::$variant,)+
                    _ => $name::$default,
                }
            }

            pub fn try_parse(value: &str) -> Option<Self> {
                match value {
                    $($value => Some($name::$variant),)+
                    _ => None,
                }
            }
        }

        impl Default for $name {
            fn default() -> Self {
                $name::$default
            }
        }

        impl ::serde::Serialize for $name {
            fn serialize<S: ::serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> ::serde::Deserialize<'de> for $name {
            fn deserialize<D: ::serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let value = <Option<String> as ::serde::Deserialize>::deserialize(deserializer)?;
                Ok(value.map(|value| $name::parse(&value)).unwrap_or_default())
            }
        }
    };
}

pub(crate) use string_enum;

string_enum!(
    /// Sens d'une opération. Les transactions du journal ne stockent que `income` et `expense` ;
    /// `transfer` sert aux échéances et aux transactions fictives.
    TransactionType, default = Expense, {
        Income => "income",
        Expense => "expense",
        Transfer => "transfer",
    }
);

impl TransactionType {
    pub fn label(self) -> &'static str {
        match self {
            TransactionType::Income => "Revenu",
            TransactionType::Expense => "Dépense",
            TransactionType::Transfer => "Virement",
        }
    }

    pub fn plural_label(self) -> &'static str {
        match self {
            TransactionType::Income => "Revenus",
            TransactionType::Expense => "Dépenses",
            TransactionType::Transfer => "Virements",
        }
    }

    pub fn color(self) -> &'static str {
        match self {
            TransactionType::Income => "#10b981",
            TransactionType::Expense => "#ef4444",
            TransactionType::Transfer => TRANSFER_COLOR,
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            TransactionType::Income => "TrendingUp",
            TransactionType::Expense => "TrendingDown",
            TransactionType::Transfer => TRANSFER_CATEGORY_ICON,
        }
    }
}

string_enum!(
    Periodicity, default = Monthly, {
        Once => "once",
        Daily => "daily",
        Weekly => "weekly",
        Biweekly => "biweekly",
        Bimonthly => "bimonthly",
        Fourweekly => "fourweekly",
        Monthly => "monthly",
        Bimestrial => "bimestrial",
        Quarterly => "quarterly",
        Fourmonthly => "fourmonthly",
        Semiannual => "semiannual",
        Annual => "annual",
        Biennial => "biennial",
    }
);

impl Periodicity {
    /// Libellé court affiché dans les listes.
    pub fn label(self) -> &'static str {
        match self {
            Periodicity::Once => "Une seule fois",
            Periodicity::Daily => "Journalier",
            Periodicity::Weekly => "Hebdomadaire",
            Periodicity::Biweekly => "Toutes les 2 semaines",
            Periodicity::Bimonthly => "Bimensuel",
            Periodicity::Fourweekly => "Toutes les 4 semaines",
            Periodicity::Monthly => "Mensuel",
            Periodicity::Bimestrial => "Bimestriel",
            Periodicity::Quarterly => "Trimestriel",
            Periodicity::Fourmonthly => "Tous les 4 mois",
            Periodicity::Semiannual => "Semestriel",
            Periodicity::Annual => "Annuel",
            Periodicity::Biennial => "Bisannuel",
        }
    }

    /// Libellé détaillé affiché dans le formulaire d'échéance.
    pub fn form_label(self) -> &'static str {
        match self {
            Periodicity::Once => "Une seule fois",
            Periodicity::Daily => "Journalière",
            Periodicity::Weekly => "Hebdomadaire",
            Periodicity::Biweekly => "Toutes les 2 semaines",
            Periodicity::Bimonthly => "Bimensuelle (2x/mois)",
            Periodicity::Fourweekly => "Toutes les 4 semaines",
            Periodicity::Monthly => "Mensuelle",
            Periodicity::Bimestrial => "Bimestrielle (tous les 2 mois)",
            Periodicity::Quarterly => "Trimestrielle (tous les 3 mois)",
            Periodicity::Fourmonthly => "Quadrimestrielle (tous les 4 mois)",
            Periodicity::Semiannual => "Semestrielle (tous les 6 mois)",
            Periodicity::Annual => "Annuelle",
            Periodicity::Biennial => "Biennale (tous les 2 ans)",
        }
    }
}

string_enum!(
    Theme, default = System, {
        Light => "light",
        Dark => "dark",
        System => "system",
    }
);

string_enum!(
    /// Plage des pages Analyses et Prédictions.
    TimeRange, default = Year, {
        Week => "week",
        Month => "month",
        TwoMonths => "2months",
        ThreeMonths => "3months",
        SixMonths => "6months",
        NineMonths => "9months",
        Year => "year",
        Custom => "custom",
    }
);

impl TimeRange {
    pub fn label(self) -> &'static str {
        match self {
            TimeRange::Week => "Semaine",
            TimeRange::Month => "Mois",
            TimeRange::TwoMonths => "2 Mois",
            TimeRange::ThreeMonths => "3 Mois",
            TimeRange::SixMonths => "6 Mois",
            TimeRange::NineMonths => "9 Mois",
            TimeRange::Year => "1 An",
            TimeRange::Custom => "Personnalisé",
        }
    }

    pub fn title_label(self) -> &'static str {
        match self {
            TimeRange::Week => "1 semaine",
            TimeRange::Month => "1 mois",
            TimeRange::TwoMonths => "2 mois",
            TimeRange::ThreeMonths => "3 mois",
            TimeRange::SixMonths => "6 mois",
            TimeRange::NineMonths => "9 mois",
            TimeRange::Year => "1 an",
            TimeRange::Custom => "personnalisée",
        }
    }

    pub fn months(self) -> Option<u32> {
        match self {
            TimeRange::Month => Some(1),
            TimeRange::TwoMonths => Some(2),
            TimeRange::ThreeMonths => Some(3),
            TimeRange::SixMonths => Some(6),
            TimeRange::NineMonths => Some(9),
            TimeRange::Year => Some(12),
            TimeRange::Week | TimeRange::Custom => None,
        }
    }
}

string_enum!(
    /// Plage d'affichage de l'échéancier.
    ScheduledDueRange, default = All, {
        All => "all",
        Month => "month",
        TwoMonths => "2months",
        ThreeMonths => "3months",
        SixMonths => "6months",
        Year => "year",
    }
);

impl ScheduledDueRange {
    pub fn label(self) -> &'static str {
        match self {
            ScheduledDueRange::All => "Toutes",
            ScheduledDueRange::Month => "Mois",
            ScheduledDueRange::TwoMonths => "2 Mois",
            ScheduledDueRange::ThreeMonths => "3 Mois",
            ScheduledDueRange::SixMonths => "6 Mois",
            ScheduledDueRange::Year => "1 An",
        }
    }

    pub fn months(self) -> Option<u32> {
        match self {
            ScheduledDueRange::All => None,
            ScheduledDueRange::Month => Some(1),
            ScheduledDueRange::TwoMonths => Some(2),
            ScheduledDueRange::ThreeMonths => Some(3),
            ScheduledDueRange::SixMonths => Some(6),
            ScheduledDueRange::Year => Some(12),
        }
    }
}

pub const ACCOUNT_TYPES: [&str; 4] = ["Courant", "Épargne", "Investissement", "Espèces"];

/// Icône et couleur proposées par défaut selon le type de compte.
pub fn account_type_defaults(account_type: &str) -> (&'static str, &'static str) {
    match account_type {
        "Épargne" => ("PiggyBank", "#10b981"),
        "Espèces" => ("Banknote", "#f59e0b"),
        "Investissement" => ("TrendingUp", "#8b5cf6"),
        _ => (DEFAULT_ACCOUNT_ICON, DEFAULT_ACCOUNT_COLOR),
    }
}

pub(crate) fn null_to_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

pub(crate) fn empty_to_none<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Option::<String>::deserialize(deserializer)?.filter(|value| !value.is_empty()))
}

fn default_account_color() -> String {
    DEFAULT_ACCOUNT_COLOR.to_string()
}

fn default_account_icon() -> String {
    DEFAULT_ACCOUNT_ICON.to_string()
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    #[serde(default, deserialize_with = "null_to_default")]
    pub name: String,
    #[serde(rename = "type", default, deserialize_with = "null_to_default")]
    pub account_type: String,
    #[serde(rename = "initialBalance", default, deserialize_with = "null_to_default")]
    pub initial_balance: f64,
    #[serde(default = "default_account_color")]
    pub color: String,
    #[serde(default = "default_account_icon")]
    pub icon: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Transaction {
    pub id: String,
    pub date: String,
    #[serde(rename = "accountId")]
    pub account_id: String,
    #[serde(rename = "type")]
    pub transaction_type: TransactionType,
    #[serde(default, deserialize_with = "null_to_default")]
    pub amount: f64,
    #[serde(default, deserialize_with = "null_to_default")]
    pub category: String,
    #[serde(default, deserialize_with = "null_to_default")]
    pub description: String,
    #[serde(default, deserialize_with = "null_to_default")]
    pub checked: bool,
    #[serde(rename = "isTransfer", default, deserialize_with = "null_to_default")]
    pub is_transfer: bool,
    #[serde(rename = "linkedTransactionId", default, deserialize_with = "empty_to_none")]
    pub linked_transaction_id: Option<String>,
}

impl Transaction {
    pub fn is_income(&self) -> bool {
        self.transaction_type == TransactionType::Income
    }

    pub fn is_transfer_category(&self) -> bool {
        self.category == TRANSFER_CATEGORY_ID
    }

    /// Type affiché : un virement lié reste une dépense ou un revenu en base.
    pub fn display_type(&self) -> TransactionType {
        if self.is_transfer_category() {
            TransactionType::Transfer
        } else if self.is_income() {
            TransactionType::Income
        } else {
            TransactionType::Expense
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Category {
    pub id: String,
    #[serde(default, deserialize_with = "null_to_default")]
    pub name: String,
    #[serde(default, deserialize_with = "null_to_default")]
    pub icon: String,
    #[serde(default, deserialize_with = "null_to_default")]
    pub color: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScheduledTransaction {
    pub id: String,
    #[serde(default, deserialize_with = "null_to_default")]
    pub description: String,
    #[serde(default, deserialize_with = "null_to_default")]
    pub amount: f64,
    #[serde(rename = "type")]
    pub transaction_type: TransactionType,
    #[serde(default)]
    pub frequency: Periodicity,
    #[serde(rename = "accountId")]
    pub account_id: String,
    #[serde(rename = "nextDate")]
    pub next_date: String,
    #[serde(default, deserialize_with = "null_to_default")]
    pub category: String,
    #[serde(rename = "toAccountId", default, deserialize_with = "empty_to_none")]
    pub to_account_id: Option<String>,
    #[serde(rename = "includeInForecast", default)]
    pub include_in_forecast: Option<bool>,
    #[serde(rename = "budgetId", default, deserialize_with = "empty_to_none")]
    pub budget_id: Option<String>,
    #[serde(rename = "endDate", default, deserialize_with = "empty_to_none")]
    pub end_date: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Budget {
    pub id: String,
    #[serde(default, deserialize_with = "null_to_default")]
    pub name: String,
    #[serde(default, deserialize_with = "null_to_default")]
    pub amount: f64,
    #[serde(default, deserialize_with = "null_to_default")]
    pub category: String,
    #[serde(rename = "accountId", default, deserialize_with = "empty_to_none")]
    pub account_id: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AppData {
    #[serde(default)]
    pub accounts: Vec<Account>,
    #[serde(default)]
    pub transactions: Vec<Transaction>,
    #[serde(default)]
    pub categories: Vec<Category>,
    #[serde(default)]
    pub scheduled: Vec<ScheduledTransaction>,
    #[serde(default)]
    pub budgets: Vec<Budget>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowPosition {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowSize {
    pub width: i32,
    pub height: i32,
}

/// Transaction simulée de la page Prédictions, stockée dans les paramètres.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PredictionFakeTransaction {
    pub id: String,
    pub date: String,
    #[serde(rename = "accountId")]
    pub account_id: String,
    #[serde(rename = "type")]
    pub transaction_type: TransactionType,
    pub amount: f64,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(
        rename = "toAccountId",
        default,
        deserialize_with = "empty_to_none",
        skip_serializing_if = "Option::is_none"
    )]
    pub to_account_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transaction_json_matches_v1_shape() {
        let transaction: Transaction = serde_json::from_str(
            r#"{"id":"t1","date":"2026-05-18","accountId":"a1","type":"expense","amount":12.5,"category":"5","description":null,"checked":true,"isTransfer":false,"linkedTransactionId":null}"#,
        )
        .unwrap();
        assert_eq!(transaction.description, "");
        assert_eq!(transaction.transaction_type, TransactionType::Expense);

        let json = serde_json::to_value(&transaction).unwrap();
        assert_eq!(json["accountId"], "a1");
        assert_eq!(json["isTransfer"], false);
        assert!(json["linkedTransactionId"].is_null());
    }

    #[test]
    fn account_defaults_match_v1() {
        let account: Account = serde_json::from_str(r#"{"id":"a","name":"Compte","type":"Courant"}"#).unwrap();
        assert_eq!(account.initial_balance, 0.0);
        assert_eq!(account.color, DEFAULT_ACCOUNT_COLOR);
        assert_eq!(account.icon, DEFAULT_ACCOUNT_ICON);
    }

    #[test]
    fn unknown_enum_values_fall_back_to_defaults() {
        assert_eq!(Periodicity::parse("weird"), Periodicity::Monthly);
        assert_eq!(TimeRange::parse("2months"), TimeRange::TwoMonths);
        assert_eq!(ScheduledDueRange::parse(""), ScheduledDueRange::All);
    }
}
