import React, { useState, useRef, useEffect, useLayoutEffect } from "react";
import { createPortal } from "react-dom";
import { ChevronDown, Search, Check, Tag } from "lucide-react";
import { ICONS } from "../../constants/icons";

export interface SelectOption {
    id: string;
    label: string;
    icon?: string;
    color?: string;
}

interface SearchableSelectProps {
    label?: string;
    value: string;
    onChange: (value: string) => void;
    options: SelectOption[];
    placeholder?: string;
    className?: string;
    disabled?: boolean;
    size?: "sm" | "md" | "lg";
}

const SearchableSelect: React.FC<SearchableSelectProps> = ({
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

    const selectedOption = options.find(opt => opt.id === value);

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

    return (
        <div className={`space-y-1.5 ${className} app-searchableselect`} ref={wrapperRef}>
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
                    className={`w-full ${sizes[size]} app-input flex items-center justify-between transition-shadow app-searchableselect-button ${disabled
                        ? 'opacity-50 cursor-not-allowed'
                        : 'focus:ring-2 focus:ring-primary-500 focus:border-primary-500'
                        }`}
                >
                <div className="flex items-center gap-2 truncate app-searchableselect-value">
                    {selectedOption ? (
                        <>
                            {selectedOption.color && (
                                <div
                                    className="w-3 h-3 rounded-full app-value-color"
                                    style={{ backgroundColor: selectedOption.color }}
                                />
                            )}
                            {renderIcon(selectedOption.icon, "w-4 h-4 app-value-icon")}
                            <span className="text-gray-900 dark:text-gray-200 group-[.retro]:text-black app-value-label">{selectedOption.label}</span>
                        </>
                    ) : (
                        <span className="text-gray-500 dark:text-gray-400 app-searchableselect-placeholder">{placeholder}</span>
                    )}
                </div>
                <ChevronDown className={`w-4 h-4 text-gray-400 transition-transform ${isOpen ? 'rotate-180' : ''} app-searchableselect-arrow`} />
            </button>

            {isOpen && createPortal(
                <div
                    ref={popupRef}
                    className="dropdown-menu fixed z-[9999] flex flex-col app-searchableselect-dropdown"
                    style={popupStyle}
                >
                    {/* Content */}
                    <div className="app-selector-bg flex flex-col scrollbar-thin scrollbar-thumb-gray-300 dark:scrollbar-thumb-gray-600 scrollbar-track-transparent overflow-hidden animate-in fade-in slide-in-from-top-2 duration-200" style={{ maxHeight: popupStyle.maxHeight || 240 }}>
                        <div className="select-header p-2 border-b border-black/[0.05] dark:border-white/10 sticky top-0 group-[.retro]:bg-[#FFF8DC] group-[.retro]:border-[#808080] app-searchableselect-search-header">
                            <div className="relative">
                                <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-400 app-searchableselect-search-icon" />
                                <input
                                    type="text"
                                    value={searchTerm}
                                    onChange={(e) => setSearchTerm(e.target.value)}
                                    className={`w-full !pl-12 pr-3 ${sizes[size]} app-input focus:outline-none focus:ring-1 focus:ring-primary-500 app-searchableselect-search-input`}
                                    placeholder="Rechercher..."
                                    autoFocus
                                />
                            </div>
                        </div>

                        <div className="overflow-y-auto flex-1 p-1 app-searchableselect-options">
                            {filteredOptions.length === 0 ? (
                                <div className="p-3 text-center text-sm text-gray-500 dark:text-gray-400 app-searchableselect-no-results">
                                    Aucun résultat
                                </div>
                            ) : (
                                filteredOptions.map(option => (
                                    <button
                                        key={option.id}
                                        type="button"
                                        onClick={() => {
                                            onChange(option.id);
                                            setIsOpen(false);
                                            setSearchTerm("");
                                        }}
                                        className={`w-full flex items-center justify-between px-3 py-2 text-sm rounded-md transition-colors cursor-pointer app-searchableselect-option ${value === option.id
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
                                        {value === option.id && <Check className="w-4 h-4 app-option-check" />}
                                    </button>
                                ))
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

export default SearchableSelect;
