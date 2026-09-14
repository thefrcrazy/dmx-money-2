import React, { useState, useRef, useEffect, useLayoutEffect } from "react";
import { createPortal } from "react-dom";
import { Calendar as CalendarIcon } from "lucide-react";
import Calendar from "./Calendar";
import { format, parseISO, isValid } from "date-fns";

interface InputProps extends Omit<React.InputHTMLAttributes<HTMLInputElement>, 'size'> {
    label?: string;
    size?: 'sm' | 'md' | 'lg';
    icon?: React.ElementType;
    rightElement?: React.ReactNode;
    containerClassName?: string;
}

const Input: React.FC<InputProps> = ({
    label,
    size = 'md',
    icon: Icon,
    rightElement,
    className = '',
    containerClassName = '',
    disabled,
    id,
    type,
    value,
    onChange,
    ...props
}) => {
    const inputId = id || (label ? `input-${label.toLowerCase().replace(/\s+/g, "-")}` : undefined);
    const [isCalendarOpen, setIsCalendarOpen] = useState(false);
    const [displayText, setDisplayText] = useState("");
    const containerRef = useRef<HTMLDivElement>(null);
    const inputRef = useRef<HTMLInputElement>(null);
    const popupRef = useRef<HTMLDivElement>(null);
    const [popupStyle, setPopupStyle] = useState<React.CSSProperties>({});

    const sizes = {
        sm: "h-8 text-xs",
        md: "h-9 text-sm",
        lg: "h-11 text-base",
    };

    const EffectiveIcon = Icon || (type === 'date' ? CalendarIcon : undefined);
    const paddingLeft = EffectiveIcon ? '!pl-10' : '!pl-3';
    const paddingRight = (rightElement || type === 'date') ? '!pr-10' : '!pr-3';

    // Sync display text with value prop (YYYY-MM-DD -> DD/MM/YYYY)
    useEffect(() => {
        if (type === 'date') {
            if (value && typeof value === 'string') {
                const date = parseISO(value);
                if (isValid(date)) {
                    // Only update if not focused to avoid cursor jumping
                    if (document.activeElement !== inputRef.current) {
                        setDisplayText(format(date, 'dd/MM/yyyy'));
                    }
                } else {
                    setDisplayText(value);
                }
            } else {
                setDisplayText('');
            }
        }
    }, [value, type]);

    useLayoutEffect(() => {
        const updatePosition = () => {
            if (isCalendarOpen && containerRef.current) {
                const rect = containerRef.current.getBoundingClientRect();
                const viewportHeight = window.innerHeight;
                const spaceBelow = viewportHeight - rect.bottom;
                const spaceAbove = rect.top;
                const popupHeight = 320;

                const showAbove = spaceBelow < popupHeight && spaceAbove > spaceBelow;
                const maxHeight = showAbove ? spaceAbove - 16 : viewportHeight - rect.bottom - 16;

                setPopupStyle({
                    position: "fixed",
                    left: rect.left,
                    ...(showAbove
                        ? { bottom: viewportHeight - rect.top + 4, maxHeight: Math.min(maxHeight, 350) }
                        : { top: rect.bottom + 4, maxHeight: Math.min(maxHeight, 350) }
                    ),
                    zIndex: 9999,
                });
            }
        };

        if (isCalendarOpen) {
            updatePosition();
            window.addEventListener("resize", updatePosition);
            window.addEventListener("scroll", updatePosition, true);
        }

        return () => {
            window.removeEventListener("resize", updatePosition);
            window.removeEventListener("scroll", updatePosition, true);
        };
    }, [isCalendarOpen]);

    // Handle clicks outside to close calendar
    useEffect(() => {
        const handleClickOutside = (event: MouseEvent) => {
            if (
                containerRef.current &&
                !containerRef.current.contains(event.target as Node) &&
                popupRef.current &&
                !popupRef.current.contains(event.target as Node)
            ) {
                setIsCalendarOpen(false);
            }
        };
        if (isCalendarOpen) {
            document.addEventListener("mousedown", handleClickOutside);
        }
        return () => document.removeEventListener("mousedown", handleClickOutside);
    }, [isCalendarOpen]);

    const handleInputChange = (e: React.ChangeEvent<HTMLInputElement>) => {
        const input = e.target;
        const selectionStart = input.selectionStart || 0;
        let val = input.value;

        if (type === 'date') {
            // On ne garde que les chiffres et les slashs
            // On autorise temporairement plus de caractères pour éviter la troncature pendant l'édition
            val = val.replace(/[^\d/]/g, '');
            
            const isDeleting = val.length < displayText.length;
            
            // Ajout automatique des slashs seulement si on ne supprime pas
            if (!isDeleting) {
                if ((val.length === 2 || val.length === 5) && !val.endsWith('/')) {
                    val += '/';
                }
            }

            // On découpe par slash pour valider chaque partie (JJ, MM, AAAA)
            const parts = val.split('/');
            const formattedParts = parts.slice(0, 3).map((part, i) => {
                if (i === 2) return part.substring(0, 4); // Année max 4
                return part.substring(0, 2); // Jour et Mois max 2
            });
            
            const finalVal = formattedParts.join('/');
            
            // Calcul de la nouvelle position du curseur
            let newPos = selectionStart;
            // Si on a ajouté un slash automatiquement, on avance le curseur
            if (!isDeleting && finalVal.length > val.length && (newPos === 2 || newPos === 5)) {
                newPos++;
            }
            // Si on a tronqué un dépassement de partie (ex: 701 -> 70), on recule si nécessaire
            if (finalVal.length < val.length && newPos > finalVal.length) {
                newPos = finalVal.length;
            }

            setDisplayText(finalVal);
            
            // Restauration de la position du curseur après le rendu
            requestAnimationFrame(() => {
                if (inputRef.current) {
                    inputRef.current.setSelectionRange(newPos, newPos);
                }
            });
            
            // Déclenche le onChange seulement pour une date complète et valide
            if (formattedParts.length === 3 && formattedParts[2].length === 4) {
                const day = formattedParts[0].padStart(2, '0');
                const month = formattedParts[1].padStart(2, '0');
                const year = formattedParts[2];
                const isoDate = `${year}-${month}-${day}`;
                
                if (isValid(parseISO(isoDate)) && onChange) {
                    const event = {
                        ...e,
                        target: { ...e.target, value: isoDate }
                    } as React.ChangeEvent<HTMLInputElement>;
                    onChange(event);
                }
            }
        } else {
            if (onChange) onChange(e);
        }
    };

    const handleBlur = (e: React.FocusEvent<HTMLInputElement>) => {
        if (type === 'date') {
            // Re-format on blur to ensure consistency
            if (value && typeof value === 'string') {
                const date = parseISO(value);
                if (isValid(date)) {
                    setDisplayText(format(date, 'dd/MM/yyyy'));
                }
            }
        }
        if (props.onBlur) props.onBlur(e);
    };

    const handleDateSelect = (date: Date) => {
        const isoDate = format(date, 'yyyy-MM-dd');
        setDisplayText(format(date, 'dd/MM/yyyy'));
        if (onChange) {
            const event = {
                target: { value: isoDate }
            } as React.ChangeEvent<HTMLInputElement>;
            onChange(event);
        }
        setIsCalendarOpen(false);
    };

    return (
        <div className={`space-y-1.5 ${containerClassName}`} ref={containerRef}>
            {label && (
                <label htmlFor={inputId} className="block text-[11px] font-bold text-gray-500 dark:text-gray-400 uppercase tracking-wider ml-1">
                    {label}
                </label>
            )}
            <div className="relative app-input-container">
                {EffectiveIcon && (
                    <div className="absolute left-3 top-1/2 -translate-y-1/2 flex items-center z-20">
                        {type === 'date' ? (
                            <button
                                type="button"
                                tabIndex={-1}
                                onClick={() => !disabled && setIsCalendarOpen(!isCalendarOpen)}
                                className="p-0.5 rounded hover:bg-gray-100 dark:hover:bg-neutral-800 text-gray-400 dark:text-gray-500 hover:text-primary-500 transition-colors cursor-pointer"
                                title="Ouvrir le calendrier"
                            >
                                <EffectiveIcon className="w-4 h-4" />
                            </button>
                        ) : (
                            <EffectiveIcon className="w-4 h-4 text-gray-400 dark:text-gray-500 pointer-events-none" />
                        )}
                    </div>
                )}
                <input
                    ref={inputRef}
                    id={inputId}
                    type={type === 'date' ? 'text' : type}
                    className={`app-input w-full ${sizes[size]} ${paddingLeft} ${paddingRight} ${className} ${disabled ? 'opacity-50 cursor-not-allowed' : ''}`}
                    disabled={disabled}
                    placeholder={type === 'date' ? 'JJ/MM/AAAA' : props.placeholder}
                    value={type === 'date' ? displayText : value}
                    onChange={handleInputChange}
                    onBlur={handleBlur}
                    {...props}
                />
                {rightElement && (
                    <div className="absolute right-3 top-1/2 -translate-y-1/2 text-gray-400 pointer-events-none app-input-right-element z-10">
                        {rightElement}
                    </div>
                )}

                {/* Custom Calendar Popover */}
                {type === "date" && isCalendarOpen && !disabled && createPortal(
                    <div
                        ref={popupRef}
                        className="fixed z-[9999]"
                        style={popupStyle}
                    >
                        <Calendar 
                            selectedDate={value && typeof value === "string" && isValid(parseISO(value)) ? parseISO(value) : new Date()} 
                            onDateSelect={handleDateSelect}
                            onClose={() => setIsCalendarOpen(false)}
                        />
                    </div>,
                    document.body
                )}
            </div>
        </div>
    );
};

export default Input;