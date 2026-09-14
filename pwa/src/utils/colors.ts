// Convert hex to RGB object
export const hexToRgb = (hex: string): { r: number; g: number; b: number } | null => {
    const result = /^#?([a-f\d]{2})([a-f\d]{2})([a-f\d]{2})$/i.exec(hex);
    return result ? {
        r: parseInt(result[1], 16),
        g: parseInt(result[2], 16),
        b: parseInt(result[3], 16)
    } : null;
};

// Convert RGB to Hex
export const rgbToHex = (r: number, g: number, b: number): string => {
    return "#" + ((1 << 24) + (r << 16) + (g << 8) + b).toString(16).slice(1);
};

/**
 * Mix two colors
 * @param color1 The color to mix towards (e.g. White or Black)
 * @param color2 The base color
 * @param weight The weight of color1 (0..1)
 */
const mix = (color1: { r: number; g: number; b: number }, color2: { r: number; g: number; b: number }, weight: number) => {
    return {
        r: Math.round(color1.r * weight + color2.r * (1 - weight)),
        g: Math.round(color1.g * weight + color2.g * (1 - weight)),
        b: Math.round(color1.b * weight + color2.b * (1 - weight)),
    };
};

// Generate Tailwind-like palette from a single color
export const generatePalette = (hexColor: string) => {
    const base = hexToRgb(hexColor);
    if (!base) return null;

    const white = { r: 255, g: 255, b: 255 };
    const black = { r: 0, g: 0, b: 0 };

    return {
        50: mix(white, base, 0.95),
        100: mix(white, base, 0.9),
        200: mix(white, base, 0.75),
        300: mix(white, base, 0.6),
        400: mix(white, base, 0.3),
        500: base, // Base color
        600: mix(black, base, 0.1), 
        700: mix(black, base, 0.25),
        800: mix(black, base, 0.45),
        900: mix(black, base, 0.65),
        950: mix(black, base, 0.85),
    };
};

// Format RGB object to CSS string "r g b" for Tailwind v3
export const formatRgb = (rgb: { r: number; g: number; b: number }) => `${rgb.r} ${rgb.g} ${rgb.b}`;