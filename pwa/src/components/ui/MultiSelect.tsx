import React, { useState, useRef, useEffect, useLayoutEffect } from "react";
import { createPortal } from "react-dom";
import { ChevronDown, Search, Check, X, Tag } from "lucide-react";
import { ICONS } from "../../constants/icons";

export interface SelectOption {
    id: string;
    label: string;
    icon?: string;
    color?: string;
}

interface MultiSelectProps {
    label?: string;
    value: string[];
    onChange: (value: string[]) => void;
    options: SelectOption[];
    placeholder?: string;
    className?: string;
    disabled?: boolean;
    size?: "sm" | "md" | "lg";
}

const MultiSelect: React.FC<MultiSelectProps> = ({
    label,
    value,
    onChange,
    options,
    placeholder = "Sélectionner...",
    className = "",
    disabled = false,
    size = "md",
}) => {
    const [isOpen, setIsOpen] = useState(false);
    const [searchTerm, setSearchTerm] = useState("");
    const wrapperRef = useRef<HTMLDivElement>(null);
    const popupRef = useRef<HTMLDivElement>(null);
    const [popupStyle, setPopupStyle] = useState<React.CSSProperties>({});

    const sizes = {
        sm: "h-8 px-3 text-xs",
        md: "h-9 px-3 text-sm",
        lg: "h-11 px-3 text-base",
    };

    useLayoutEffect(() => {
        const updatePosition = () => {
            if (isOpen && wrapperRef.current) {
                const rect = wrapperRef.current.getBoundingClientRect();
                const viewportHeight = window.innerHeight;
                const spaceBelow = viewportHeight - rect.bottom;
                const spaceAbove = rect.top;
                const popupHeight = 240;

                const showAbove = spaceBelow < popupHeight && spaceAbove > spaceBelow;
                const maxHeight = showAbove ? spaceAbove - 16 : viewportHeight - rect.bottom - 16;

                setPopupStyle({
                    position: "fixed",
                    left: rect.left,
                    width: rect.width,
                    ...(showAbove
                        ? { bottom: viewportHeight - rect.top + 4, maxHeight: Math.min(maxHeight, 240) }
                        : { top: rect.bottom + 4, maxHeight: Math.min(maxHeight, 240) }
                    ),
                    zIndex: 9999,
                    display: "flex",
                    flexDirection: "column",
                });
            }
        };

        if (isOpen) {
            updatePosition();
            window.addEventListener("resize", updatePosition);
            window.addEventListener("scroll", updatePosition, true);
        }

        return () => {
            window.removeEventListener("resize", updatePosition);
            window.removeEventListener("scroll", updatePosition, true);
        };
    }, [isOpen]);

    useEffect(() => {
        const handleClickOutside = (event: MouseEvent) => {
            if (
                wrapperRef.current &&
                !wrapperRef.current.contains(event.target as Node) &&
                popupRef.current &&
                !popupRef.current.contains(event.target as Node)
            ) {
                setIsOpen(false);
            }
        };
        document.addEventListener("mousedown", handleClickOutside);
        return () => document.removeEventListener("mousedown", handleClickOutside);
    }, []);

    const filteredOptions = options.filter(opt =>
        opt.label.toLowerCase().includes(searchTerm.toLowerCase())
    );

    const renderIcon = (iconName?: string, className: string = "w-4 h-4") => {
        if (!iconName) return null;
        const Icon = ICONS[iconName] || Tag;
        return <Icon className={className} />;
    };

    const handleSelect = (optionId: string) => {
        if (value.includes(optionId)) {
            onChange(value.filter(id => id !== optionId));
        } else {
            onChange([...value, optionId]);
        }
    };

    const removeOption = (e: React.MouseEvent, optionId: string) => {
        e.stopPropagation();
        onChange(value.filter(id => id !== optionId));
    };

    const selectedOptions = options.filter(opt => value.includes(opt.id));

    return (
        <div className={`space-y-1.5 ${className} app-multiselect`} ref={wrapperRef}>
            {label && (
                <label className="block text-[11px] font-bold text-gray-500 dark:text-gray-400 uppercase tracking-wider ml-1">
                    {label}
                </label>
            )}
            <div className="relative">
                <button
                    type="button"
                    onClick={() => !disabled && setIsOpen(!isOpen)}
                    disabled={disabled}
                    className={`w-full ${sizes[size]} app-input flex items-center justify-between transition-shadow app-multiselect-button ${disabled
                        ? 'opacity-50 cursor-not-allowed'
                        : 'focus:ring-2 focus:ring-primary-500 focus:border-primary-500'
                        }`}
                >
                <div className="flex flex-nowrap gap-1.5 items-center overflow-hidden app-multiselect-value">
                    {selectedOptions.length > 0 ? (
                        <>
                            {/* Show first item */}
                            <div
                                className="flex items-center gap-1 px-2 py-0.5 rounded-md bg-primary-50 dark:bg-primary-900/30 text-xs font-medium text-primary-700 dark:text-primary-300 border border-primary-100 dark:border-primary-800 whitespace-nowrap app-multiselect-tag"
                            >
                                {selectedOptions[0].color && (
                                    <div
                                        className="w-1.5 h-1.5 rounded-full app-tag-color"
                                        style={{ backgroundColor: selectedOptions[0].color }}
                                    />
                                )}
                                <span className="truncate max-w-[100px] app-tag-label">{selectedOptions[0].label}</span>
                                <div
                                    role="button"
                                    onClick={(e) => removeOption(e, selectedOptions[0].id)}
                                    className="hover:text-primary-900 dark:hover:text-primary-100 app-tag-remove"
                                >
                                    <X className="w-3 h-3" />
                                </div>
                            </div>

                            {/* Show count for others */}
                            {selectedOptions.length > 1 && (
                                <div className="flex items-center px-2 py-0.5 rounded-md bg-gray-100 dark:bg-neutral-800 text-xs font-medium text-gray-600 dark:text-gray-300 border border-gray-200 dark:border-neutral-700 whitespace-nowrap app-multiselect-count">
                                    +{selectedOptions.length - 1}
                                </div>
                            )}
                        </>
                    ) : (
                        <span className="text-gray-500 dark:text-gray-400 truncate app-multiselect-placeholder">{placeholder}</span>
                    )}
                </div>
                <ChevronDown className={`w-4 h-4 text-gray-400 transition-transform ${isOpen ? 'rotate-180' : ''} ml-2 shrink-0 app-multiselect-arrow`} />
            </button>

            {isOpen && createPortal(
                <div
                    ref={popupRef}
                    className="dropdown-menu fixed z-[9999] flex flex-col app-multiselect-dropdown"
                    style={popupStyle}
                >
                    {/* Content */}
                    <div className="app-selector-bg flex flex-col scrollbar-thin scrollbar-thumb-gray-300 dark:scrollbar-thumb-gray-600 scrollbar-track-transparent overflow-hidden animate-in fade-in slide-in-from-top-2 duration-200" style={{ maxHeight: popupStyle.maxHeight || 240 }}>
                        <div className="select-header p-2 border-b border-black/[0.05] dark:border-white/10 sticky top-0 group-[.retro]:bg-[#FFF8DC] group-[.retro]:border-[#808080] app-multiselect-search-header">
                            <div className="relative">
                                <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-400 app-multiselect-search-icon" />
                                <input
                                    type="text"
                                    value={searchTerm}
                                    onChange={(e) => setSearchTerm(e.target.value)}
                                    className={`w-full !pl-12 pr-3 ${sizes[size]} app-input focus:outline-none focus:ring-1 focus:ring-primary-500 app-multiselect-search-input`}
                                    placeholder="Rechercher..."
                                    autoFocus
                                />
                            </div>
                        </div>

                        <div className="overflow-y-auto flex-1 p-1 app-multiselect-options">
                            {filteredOptions.length === 0 ? (
                                <div className="p-3 text-center text-sm text-gray-500 dark:text-gray-400 app-multiselect-no-results">
                                    Aucun résultat
                                </div>
                            ) : (
                                filteredOptions.map(option => {
                                    const isSelected = value.includes(option.id);
                                    return (
                                        <button
                                            key={option.id}
                                            type="button"
                                            onClick={() => handleSelect(option.id)}
                                            className={`w-full flex items-center justify-between px-3 py-2 text-sm rounded-md transition-colors cursor-pointer app-multiselect-option ${isSelected
                                                ? "bg-primary-100 dark:bg-primary-900 text-primary-700 dark:text-primary-300 app-option-selected"
                                                : "text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700"
                                                }`}
                                        >
                                            <div className="flex items-center gap-2 app-option-content">
                                                {option.color && (
                                                    <div
                                                        className="w-2 h-2 rounded-full app-option-color"
                                                        style={{ backgroundColor: option.color }}
                                                    />
                                                )}
                                                {renderIcon(option.icon, "w-4 h-4 app-option-icon")}
                                                <span className="app-option-label">{option.label}</span>
                                            </div>
                                            {isSelected && <Check className="w-4 h-4 app-option-check" />}
                                        </button>
                                    );
                                })
                            )}
                        </div>
                    </div>
                </div>,
                document.body
            )}
            </div>
        </div>
    );
};

export default MultiSelect;
