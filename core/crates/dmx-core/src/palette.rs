//! Constantes visuelles partagées par toutes les interfaces (port de `src/constants/icons.ts`).

pub use crate::settings::ACCENT_COLORS;

/// Icônes proposées pour les catégories et les comptes, dans l'ordre du sélecteur 1.x.
/// Les noms sont ceux de Lucide et sont stockés tels quels en base.
pub const ICON_PICKER: [&str; 92] = [
    // Transport
    "Car",
    "Plane",
    "Train",
    "Bus",
    "Bike",
    "Ship",
    "MapPin",
    "Navigation",
    "Fuel",
    // Maison
    "Home",
    "Zap",
    "Droplets",
    "Flame",
    "Wifi",
    "Sofa",
    "Bed",
    "Bath",
    "Hammer",
    "Wrench",
    // Alimentation
    "ShoppingBag",
    "Utensils",
    "Coffee",
    "Beer",
    "Wine",
    "Pizza",
    "Apple",
    "Carrot",
    // Tech et loisirs
    "Gamepad2",
    "Music",
    "Monitor",
    "Smartphone",
    "Headphones",
    "Camera",
    "Video",
    "Tv",
    "Laptop",
    "Speaker",
    "Bluetooth",
    "Battery",
    "Cpu",
    "Database",
    "Server",
    // Finance et travail
    "Briefcase",
    "Landmark",
    "CreditCard",
    "Banknote",
    "Wallet",
    "PiggyBank",
    "TrendingUp",
    "TrendingDown",
    "Activity",
    "Target",
    "Award",
    // Santé
    "Heart",
    "Stethoscope",
    "Pill",
    "Dumbbell",
    "User",
    "Users",
    "Baby",
    // Divers
    "Tag",
    "Book",
    "Gift",
    "Smile",
    "Frown",
    "Meh",
    "Sun",
    "Moon",
    "Cloud",
    "Umbrella",
    "Snowflake",
    "Star",
    "Bell",
    "Key",
    "Lock",
    "Shield",
    // Interface
    "Search",
    "Filter",
    "Settings",
    "Menu",
    "X",
    "Plus",
    "Minus",
    "Trash2",
    "Edit2",
    "Check",
    "CheckCircle2",
    "Circle",
    "ChevronDown",
    "ArrowRightLeft",
    "Clock",
    "Calendar",
];

/// Nuancier des comptes et catégories : 12 familles × 10 nuances, de la plus sombre à la plus claire.
pub const CATEGORY_COLORS: [&str; 120] = [
    "#000000", "#7f1d1d", "#7c2d12", "#78350f", "#365314", "#14532d", "#134e4a", "#164e63", "#1e3a8a", "#312e81",
    "#581c87", "#831843", "#111827", "#991b1b", "#9a3412", "#92400e", "#3f6212", "#166534", "#115e59", "#155e75",
    "#1e40af", "#3730a3", "#6b21a8", "#9d174d", "#1f2937", "#b91c1c", "#c2410c", "#b45309", "#4d7c0f", "#15803d",
    "#0f766e", "#0e7490", "#1d4ed8", "#4338ca", "#7e22ce", "#be185d", "#374151", "#dc2626", "#ea580c", "#d97706",
    "#65a30d", "#16a34a", "#0d9488", "#0891b2", "#2563eb", "#4f46e5", "#9333ea", "#db2777", "#4b5563", "#ef4444",
    "#f97316", "#f59e0b", "#84cc16", "#22c55e", "#14b8a6", "#06b6d4", "#3b82f6", "#6366f1", "#a855f7", "#ec4899",
    "#6b7280", "#f87171", "#fb923c", "#fbbf24", "#a3e635", "#4ade80", "#2dd4bf", "#22d3ee", "#60a5fa", "#818cf8",
    "#c084fc", "#f472b6", "#9ca3af", "#fca5a5", "#fdba74", "#fcd34d", "#bef264", "#86efac", "#5eead4", "#67e8f9",
    "#93c5fd", "#a5b4fc", "#d8b4fe", "#f9a8d4", "#d1d5db", "#fecaca", "#fed7aa", "#fde68a", "#d9f99d", "#bbf7d0",
    "#99f6e4", "#a5f3fc", "#bfdbfe", "#c7d2fe", "#e9d5ff", "#fbcfe8", "#e5e7eb", "#fee2e2", "#ffedd5", "#fef3c7",
    "#ecfccb", "#dcfce7", "#ccfbf1", "#cffafe", "#dbeafe", "#e0e7ff", "#f3e8ff", "#fce7f3", "#ffffff", "#fef2f2",
    "#fff7ed", "#fffbeb", "#f7fee7", "#f0fdf4", "#f0fdfa", "#ecfeff", "#eff6ff", "#eef2ff", "#faf5ff", "#fdf2f8",
];

/// Couleur d'accent par défaut de l'interface (indigo 1.x).
pub const DEFAULT_ACCENT_COLOR: &str = "#6366f1";
pub const INCOME_COLOR: &str = "#10b981";
pub const EXPENSE_COLOR: &str = "#ef4444";
pub const CHECKED_BALANCE_COLOR: &str = "#059669";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picker_icons_exist_in_shared_assets() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../shared/icons/lucide");
        for icon in ICON_PICKER {
            assert!(dir.join(format!("{icon}.svg")).exists(), "icône manquante : {icon}");
        }
        for (_, _, icon, _) in crate::seed::DEFAULT_CATEGORIES {
            assert!(dir.join(format!("{icon}.svg")).exists(), "icône manquante : {icon}");
        }
    }
}
