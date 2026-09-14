import {
    Tag, ShoppingBag, Utensils, Car, Home, Zap, Heart, Briefcase, Plane, Gamepad2, Music, Book, Gift, Coffee, Smartphone,
    Landmark, CreditCard, Banknote, Wallet, PiggyBank, TrendingUp, TrendingDown, Activity, Target, Award,
    Smile, Frown, Meh, Sun, Moon, Cloud, Umbrella, Snowflake, Droplets, Flame,
    Monitor, Headphones, Camera, Video, Wifi, Bluetooth, Battery, Cpu, Database, Server,
    Train, Bus, Bike, Ship, MapPin, Navigation, Fuel,
    Sofa, Bed, Bath, Hammer, Wrench,
    Beer, Wine, Pizza, Apple, Carrot,
    Tv, Laptop, Speaker,
    Stethoscope, Pill, Dumbbell, User, Users, Baby,
    Star, Bell, Key, Lock, Shield,
    Search, Filter, Settings, Menu, X, Plus, Minus, Trash2, Edit2, Check, CheckCircle2, Circle, ChevronDown, ArrowRightLeft, Clock, Calendar
} from 'lucide-react';

export const ICONS: Record<string, React.ElementType> = {
    // Transport
    Car, Plane, Train, Bus, Bike, Ship, MapPin, Navigation, Fuel,
    // Home & Living
    Home, Zap, Droplets, Flame, Wifi, Sofa, Bed, Bath, Hammer, Wrench,
    // Food & Drink
    ShoppingBag, Utensils, Coffee, Beer, Wine, Pizza, Apple, Carrot,
    // Tech & Entertainment
    Gamepad2, Music, Monitor, Smartphone, Headphones, Camera, Video, Tv, Laptop, Speaker,
    Bluetooth, Battery, Cpu, Database, Server,
    // Finance & Work
    Briefcase, Landmark, CreditCard, Banknote, Wallet, PiggyBank, TrendingUp, TrendingDown, Activity, Target, Award,
    // Health & Wellness
    Heart, Stethoscope, Pill, Dumbbell, User, Users, Baby,
    // Misc
    Tag, Book, Gift, Smile, Frown, Meh, Sun, Moon, Cloud, Umbrella, Snowflake, Star, Bell, Key, Lock, Shield,
    // UI
    Search, Filter, Settings, Menu, X, Plus, Minus, Trash2, Edit2, Check, CheckCircle2, Circle, ChevronDown, ArrowRightLeft, Clock, Calendar
};

// 12 Families: Gray, Red, Orange, Amber, Lime, Green, Teal, Cyan, Blue, Indigo, Purple, Pink
// Ordered Row-by-Row: Row 0 = Darkest shades, Row 9 = Lightest shades
export const COLORS = [
    // Row 0 (Darkest)
    '#000000', '#7f1d1d', '#7c2d12', '#78350f', '#365314', '#14532d', '#134e4a', '#164e63', '#1e3a8a', '#312e81', '#581c87', '#831843',
    // Row 1
    '#111827', '#991b1b', '#9a3412', '#92400e', '#3f6212', '#166534', '#115e59', '#155e75', '#1e40af', '#3730a3', '#6b21a8', '#9d174d',
    // Row 2
    '#1f2937', '#b91c1c', '#c2410c', '#b45309', '#4d7c0f', '#15803d', '#0f766e', '#0e7490', '#1d4ed8', '#4338ca', '#7e22ce', '#be185d',
    // Row 3
    '#374151', '#dc2626', '#ea580c', '#d97706', '#65a30d', '#16a34a', '#0d9488', '#0891b2', '#2563eb', '#4f46e5', '#9333ea', '#db2777',
    // Row 4
    '#4b5563', '#ef4444', '#f97316', '#f59e0b', '#84cc16', '#22c55e', '#14b8a6', '#06b6d4', '#3b82f6', '#6366f1', '#a855f7', '#ec4899',
    // Row 5
    '#6b7280', '#f87171', '#fb923c', '#fbbf24', '#a3e635', '#4ade80', '#2dd4bf', '#22d3ee', '#60a5fa', '#818cf8', '#c084fc', '#f472b6',
    // Row 6
    '#9ca3af', '#fca5a5', '#fdba74', '#fcd34d', '#bef264', '#86efac', '#5eead4', '#67e8f9', '#93c5fd', '#a5b4fc', '#d8b4fe', '#f9a8d4',
    // Row 7
    '#d1d5db', '#fecaca', '#fed7aa', '#fde68a', '#d9f99d', '#bbf7d0', '#99f6e4', '#a5f3fc', '#bfdbfe', '#c7d2fe', '#e9d5ff', '#fbcfe8',
    // Row 8
    '#e5e7eb', '#fee2e2', '#ffedd5', '#fef3c7', '#ecfccb', '#dcfce7', '#ccfbf1', '#cffafe', '#dbeafe', '#e0e7ff', '#f3e8ff', '#fce7f3',
    // Row 9 (Lightest)
    '#ffffff', '#fef2f2', '#fff7ed', '#fffbeb', '#f7fee7', '#f0fdf4', '#f0fdfa', '#ecfeff', '#eff6ff', '#eef2ff', '#faf5ff', '#fdf2f8',
];
