import React from 'react';

const TitleBar: React.FC = () => {
    return (
        <div
            data-tauri-drag-region
            className="app-titlebar h-10 flex items-center justify-center fixed top-0 left-0 right-0 z-[100] select-none bg-transparent pointer-events-none"
        >
            {/* Zone de drag uniquement. Sur toutes les plateformes, on utilise désormais les contrôles natifs du système */}
        </div>
    );
};

export default TitleBar;
