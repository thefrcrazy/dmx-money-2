import DmxKit
import SwiftUI

/// Correspondance entre les icônes stockées en base (noms Lucide, identiques sur les trois
/// plateformes) et les SF Symbols, pour que la variante modern reste une app macOS native.
/// Les noms en base ne changent pas : seul l'affichage diffère.
enum Symbols {
    static func name(for lucide: String) -> String {
        map[lucide] ?? "tag"
    }

    static func image(_ lucide: String, size: CGFloat? = nil) -> Image {
        Image(systemName: name(for: lucide))
    }

    /// Icône d'une page de la barre latérale.
    static func route(_ route: AppRoute) -> String {
        switch route {
        case .dashboard: return "square.grid.2x2"
        case .accounts: return "creditcard"
        case .transactions: return "list.bullet.rectangle.portrait"
        case .budget: return "chart.bar.horizontal.page"
        case .scheduled: return "calendar.badge.clock"
        case .analytics: return "chart.pie"
        case .predictions: return "chart.line.uptrend.xyaxis"
        case .categories: return "tag"
        case .settings: return "gearshape"
        }
    }

    private static let map: [String: String] = [
        // Icônes d'interface (filtres, états) : elles arrivent aussi en noms Lucide.
        "Circle": "circle", "CircleOff": "circle.slash", "CheckCircle2": "checkmark.circle.fill",
        "Folder": "folder", "Tray": "tray",
        "AlertTriangle": "exclamationmark.triangle", "X": "xmark", "ArrowRight": "arrow.right",
        "CircleDot": "smallcircle.filled.circle",
        // Transport
        "Car": "car", "Plane": "airplane", "Train": "tram", "Bus": "bus", "Bike": "bicycle",
        "Ship": "ferry", "MapPin": "mappin.and.ellipse", "Navigation": "location", "Fuel": "fuelpump",
        // Maison
        "Home": "house", "Zap": "bolt", "Droplets": "drop", "Flame": "flame", "Wifi": "wifi",
        "Sofa": "sofa", "Bed": "bed.double", "Bath": "bathtub", "Hammer": "hammer", "Wrench": "wrench.adjustable",
        // Alimentation
        "ShoppingBag": "bag", "Utensils": "fork.knife", "Coffee": "cup.and.saucer", "Beer": "mug",
        "Wine": "wineglass", "Pizza": "fork.knife.circle", "Apple": "apple.logo", "Carrot": "carrot",
        // Tech et loisirs
        "Gamepad2": "gamecontroller", "Music": "music.note", "Monitor": "display", "Smartphone": "iphone",
        "Headphones": "headphones", "Camera": "camera", "Video": "video", "Tv": "tv", "Laptop": "laptopcomputer",
        "Speaker": "hifispeaker", "Bluetooth": "dot.radiowaves.right", "Battery": "battery.100",
        "Cpu": "cpu", "Database": "externaldrive", "Server": "server.rack",
        // Finance et travail
        "Briefcase": "briefcase", "Landmark": "building.columns", "CreditCard": "creditcard",
        "Banknote": "banknote", "Wallet": "wallet.bifold", "PiggyBank": "dollarsign.circle",
        "TrendingUp": "chart.line.uptrend.xyaxis", "TrendingDown": "chart.line.downtrend.xyaxis",
        "Activity": "waveform.path.ecg", "Target": "target", "Award": "rosette",
        // Santé
        "Heart": "heart", "Pill": "pills", "Stethoscope": "stethoscope", "Dumbbell": "dumbbell",
        "Smile": "face.smiling", "Baby": "figure.and.child.holdinghands", "Dog": "pawprint", "Cat": "pawprint",
        // Éducation et divers
        "GraduationCap": "graduationcap", "BookOpen": "book", "Book": "book.closed", "Pencil": "pencil",
        "Palette": "paintpalette", "Scissors": "scissors", "Shirt": "tshirt", "Gift": "gift",
        "Shield": "shield", "Lock": "lock", "Key": "key", "Bell": "bell", "Calendar": "calendar",
        "Clock": "clock", "Mail": "envelope", "Phone": "phone", "Users": "person.2", "User": "person",
        "Building": "building.2", "Store": "storefront", "Package": "shippingbox", "Truck": "truck.box",
        "Recycle": "arrow.3.trianglepath", "Leaf": "leaf", "Sun": "sun.max", "Moon": "moon",
        "CloudRain": "cloud.rain", "Snowflake": "snowflake", "Umbrella": "umbrella",
        "Tag": "tag", "MoreHorizontal": "ellipsis", "Star": "star", "Flag": "flag",
        "Trash2": "trash", "Plus": "plus", "Minus": "minus", "Check": "checkmark",
        "ArrowRightLeft": "arrow.left.arrow.right", "Repeat": "repeat", "Sparkles": "sparkles",
    ]
}
