export default {
    plugins: {
        'postcss-preset-env': {
            // Indiquez les navigateurs cibles (identique à votre build Vite)
            browsers: ['defaults', 'safari 13'],

            // Activez toutes les transformations nécessaires
            features: {
                'nesting-rules': true,
                'is-pseudo-class': true, // Transforme :is()
                'gap-properties': true, // Transforme gap en margin pour Flexbox (Safari 13 bug)
                'custom-properties': false, // Laissez à false si vous voulez garder les variables CSS (Safari 13 les supporte)
            },
            autoprefixer: {
                grid: 'autoplace', // Aide pour les vieilles grilles CSS
            },
        },
    },
};
