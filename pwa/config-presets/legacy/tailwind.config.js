/** @type {import('tailwindcss').Config} */

// Helper function to create color with opacity support for old WebKit (Catalina)
function withOpacityValue(variable) {
    return ({ opacityValue }) => {
        if (opacityValue === undefined) {
            return `rgb(var(${variable}))`
        }
        // Legacy syntax: rgba(R, G, B, alpha) - variable must contain commas
        return `rgba(var(${variable}), ${opacityValue})`
    }
}

export default {
    content: [
        "./index.html",
        "./src/**/*.{js,ts,jsx,tsx}",
    ],
    darkMode: 'class',
    theme: {
        extend: {
            colors: {
                primary: {
                    50: withOpacityValue('--color-primary-50-rgb'),
                    100: withOpacityValue('--color-primary-100-rgb'),
                    200: withOpacityValue('--color-primary-200-rgb'),
                    300: withOpacityValue('--color-primary-300-rgb'),
                    400: withOpacityValue('--color-primary-400-rgb'),
                    500: withOpacityValue('--color-primary-500-rgb'),
                    600: withOpacityValue('--color-primary-600-rgb'),
                    700: withOpacityValue('--color-primary-700-rgb'),
                    800: withOpacityValue('--color-primary-800-rgb'),
                    900: withOpacityValue('--color-primary-900-rgb'),
                    950: withOpacityValue('--color-primary-950-rgb'),
                },
                gray: {
                    50: withOpacityValue('--color-gray-50-rgb'),
                    100: withOpacityValue('--color-gray-100-rgb'),
                    200: withOpacityValue('--color-gray-200-rgb'),
                    300: withOpacityValue('--color-gray-300-rgb'),
                    400: withOpacityValue('--color-gray-400-rgb'),
                    500: withOpacityValue('--color-gray-500-rgb'),
                    600: withOpacityValue('--color-gray-600-rgb'),
                    700: withOpacityValue('--color-gray-700-rgb'),
                    800: withOpacityValue('--color-gray-800-rgb'),
                    900: withOpacityValue('--color-gray-900-rgb'),
                    950: withOpacityValue('--color-gray-950-rgb'),
                },
                neutral: {
                    50: withOpacityValue('--color-neutral-50-rgb'),
                    100: withOpacityValue('--color-neutral-100-rgb'),
                    200: withOpacityValue('--color-neutral-200-rgb'),
                    300: withOpacityValue('--color-neutral-300-rgb'),
                    400: withOpacityValue('--color-neutral-400-rgb'),
                    500: withOpacityValue('--color-neutral-500-rgb'),
                    600: withOpacityValue('--color-neutral-600-rgb'),
                    700: withOpacityValue('--color-neutral-700-rgb'),
                    800: withOpacityValue('--color-neutral-800-rgb'),
                    900: withOpacityValue('--color-neutral-900-rgb'),
                    950: withOpacityValue('--color-neutral-950-rgb'),
                },
            },
            fontFamily: {
                sans: ['var(--font-sans)'],
                serif: ['var(--font-serif)'],
                mono: ['var(--font-mono)'],
            },
            borderRadius: {
                'card': 'var(--card-radius)',
            },
        },
    },
    plugins: [],
}
