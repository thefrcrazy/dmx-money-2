export default {
    plugins: {
        tailwindcss: {},
        autoprefixer: {},
        'postcss-preset-env': {
            browsers: ['defaults', 'safari 13'],
            features: {
                'nesting-rules': true,
                'is-pseudo-class': true,
                'gap-properties': true,
                'custom-properties': false,
            },
            autoprefixer: {
                grid: 'autoplace',
            },
        },
    },
};
