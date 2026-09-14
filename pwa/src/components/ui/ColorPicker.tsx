import React, { useState, useRef, useEffect, useLayoutEffect } from 'react';
import { createPortal } from 'react-dom';
import { Check, ChevronDown } from 'lucide-react';

interface ColorPickerProps {
    value: string;
    onChange: (color: string) => void;
    colors: string[];
    size?: 'sm' | 'md' | 'lg';
}

const ColorPicker: React.FC<ColorPickerProps> = ({ value, onChange, colors, size = 'md' }) => {
    const [isOpen, setIsOpen] = useState(false);
    const containerRef = useRef<HTMLDivElement>(null);
    const popupRef = useRef<HTMLDivElement>(null);

    const sizes = {
        sm: "h-8 text-xs",
        md: "h-9 text-sm",
        lg: "h-11 text-base",
    };

    const padding = {
        sm: "p-1",
        md: "p-1.5",
        lg: "p-2",
    };

    // Close when clicking outside
    useEffect(() => {
        const handleClickOutside = (event: MouseEvent) => {
            if (
                containerRef.current &&
                !containerRef.current.contains(event.target as Node) &&
                popupRef.current &&
                !popupRef.current.contains(event.target as Node)
            ) {
                setIsOpen(false);
            }
        };

        if (isOpen) {
            document.addEventListener('mousedown', handleClickOutside);
        }

        return () => {
            document.removeEventListener('mousedown', handleClickOutside);
        };
    }, [isOpen]);

    const [popupStyle, setPopupStyle] = useState<React.CSSProperties>({});

    useLayoutEffect(() => {
        if (isOpen && containerRef.current) {
            const rect = containerRef.current.getBoundingClientRect();
            const viewportHeight = window.innerHeight;
            const spaceBelow = viewportHeight - rect.bottom;
            const spaceAbove = rect.top;
            const popupHeight = 320; // Approximate height

            // Prefer showing below if space permits, otherwise check above
            const showAbove = spaceBelow < popupHeight && spaceAbove > spaceBelow;

            // Calculate max height to fit in available space
            const maxHeight = showAbove ? spaceAbove - 16 : viewportHeight - rect.bottom - 16;

            setPopupStyle({
                position: 'fixed',
                left: rect.left,
                width: rect.width,
                ...(showAbove
                    ? { bottom: viewportHeight - rect.top + 8, maxHeight: Math.min(maxHeight, 400) }
                    : { top: rect.bottom + 8, maxHeight: Math.min(maxHeight, 400) }
                ),
                zIndex: 9999,
                overflowY: 'auto', // Enable scrolling if constrained
            });
        }
    }, [isOpen]);

    const handleSelect = (color: string) => {
        console.log('ColorPicker: handleSelect called with', color);
        onChange(color);
        setIsOpen(false);
    };

    return (
        <div className="relative app-colorpicker" ref={containerRef}>
            <button
                type="button"
                onClick={() => setIsOpen(!isOpen)}
                className={`w-full flex items-center justify-between app-input transition-shadow app-colorpicker-button ${sizes[size]} ${padding[size]} ${isOpen ? 'ring-2 ring-primary-500 border-primary-500' : ''}`}
            >
                <div className="flex-1 flex items-center gap-2 h-full app-colorpicker-preview">
                    <div
                        className="flex-1 h-full rounded-md border border-gray-200 dark:border-neutral-600 app-color-preview-box"
                        style={{ backgroundColor: value }}
                    />
                </div>
                <ChevronDown className={`w-4 h-4 text-gray-400 mx-2 transition-transform ${isOpen ? 'rotate-180' : ''} app-colorpicker-arrow`} />
            </button>

            {isOpen && createPortal(
                <div
                    ref={popupRef}
                    className="p-2 app-card shadow-xl animate-in fade-in zoom-in-95 duration-100 app-colorpicker-popup"
                    style={popupStyle}
                >
                    <div className="grid grid-cols-12 gap-2 app-colorpicker-grid">
                        {colors.map((color) => (
                            <button
                                key={color}
                                type="button"
                                onClick={() => handleSelect(color)}
                                className={`w-5 h-5 rounded-md flex items-center justify-center transition-all hover:scale-110 focus:outline-none focus:ring-2 focus:ring-offset-1 focus:ring-offset-white dark:focus:ring-offset-gray-900 app-color-swatch ${value === color ? 'ring-2 ring-offset-1 ring-offset-white dark:ring-offset-gray-900 ring-gray-400 dark:ring-gray-500 scale-110 z-10 app-swatch-selected' : ''
                                    } ${color === '#ffffff' ? 'border border-gray-200 dark:border-neutral-700' : ''}`}
                                style={{ backgroundColor: color }}
                                title={color}
                            >
                                {value === color && (
                                    <Check className={`w-3 h-3 ${['#ffffff', '#f3f4f6', '#e5e7eb', '#fef2f2', '#fff7ed', '#fffbeb', '#f7fee7', '#f0fdf4', '#ecfdf5', '#f0fdfa', '#ecfeff', '#f0f9ff', '#eff6ff', '#eef2ff', '#f5f3ff', '#faf5ff', '#fdf4ff', '#fdf2f8', '#fff1f2'].includes(color) ? 'text-gray-900' : 'text-white'} drop-shadow-sm app-swatch-check`} strokeWidth={3} />
                                )}
                            </button>
                        ))}
                    </div>
                </div>,
                document.body
            )}
        </div>
    );
};

export default ColorPicker;
