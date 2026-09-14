#!/bin/bash
# =============================================================================
# TailwindCSS Version Switcher
# Switch between Legacy (Catalina/Intel) and Modern (v4) environments
# =============================================================================

MODE=$1

if [ -z "$MODE" ]; then
    echo "Usage: ./switch-tailwind.sh [legacy|modern]"
    exit 1
fi

PRESETS_DIR="config-presets"

swap_configs() {
    local target=$1
    local source_dir="${PRESETS_DIR}/${target}"
    
    echo "🔄 Loading $target configuration..."

    # 1. Definir les fichiers attendus pour ce mode
    if [ "$target" == "legacy" ]; then
        FILES=("index.css:src/index.css" "vite.config.ts:vite.config.ts" "postcss.config.js:postcss.config.js" "tailwind.config.js:tailwind.config.js")
    else
        FILES=("index.css:src/index.css" "vite.config.ts:vite.config.ts")
        # Nettoyer les fichiers v3 en mode modern
        rm -f postcss.config.js tailwind.config.js
        echo "   🗑️  Cleaned up v3 config files (postcss.config.js, tailwind.config.js)"
    fi

    for mapping in "${FILES[@]}"; do
        IFS=':' read -r src dest <<< "$mapping"
        source_file="${source_dir}/${src}"
        
        if [ -f "$source_file" ]; then
            cp "$source_file" "$dest"
            echo "   ✓ Applied ${src} → ${dest}"
        fi
    done
}

if [ "$MODE" == "legacy" ]; then
    swap_configs "legacy"
    bun remove @tailwindcss/vite 2>/dev/null || true
    bun add -d tailwindcss@3.4.17 postcss autoprefixer
    
elif [ "$MODE" == "modern" ]; then
    swap_configs "modern"
    bun remove tailwindcss autoprefixer 2>/dev/null || true
    # Note: on garde postcss car il est en devDependencies mais on n'utilise pas postcss.config.js
    bun add -d tailwindcss@latest @tailwindcss/vite@latest
else
    echo "Unknown mode: $MODE"
    exit 1
fi
