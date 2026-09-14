import React from 'react';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export interface Column<T> {
    header: React.ReactNode;
    accessor?: keyof T;
    render?: (item: T) => React.ReactNode;
    className?: string;
    headerClassName?: string;
    align?: 'left' | 'center' | 'right';
    width?: string;
    minWidth?: string; // Nouvelle propriété pour la sécurité d'affichage
    truncate?: boolean;
    editable?: boolean;
    editType?: 'text' | 'number' | 'date';
}

interface TableProps<T> {
    data: T[];
    columns: Column<T>[];
    keyExtractor: (item: T) => string | number;
    emptyMessage?: React.ReactNode;
    onRowClick?: (item: T) => void;
    rowClassName?: (item: T) => string;
    onCellUpdate?: (item: T, accessor: keyof T, newValue: any) => void;
    // Selection support
    selectedIds?: Set<string | number>;
    onSelectRow?: (id: string | number) => void;
    onSelectAll?: () => void;
    isAllSelected?: boolean;
    virtualized?: boolean;
    rowHeight?: number;
    overscan?: number;
}

const TruncatedTooltip: React.FC<{ 
    children: React.ReactNode, 
    tooltipText: string, 
    align?: 'left' | 'center' | 'right' 
}> = ({ children, tooltipText, align }) => {
    const containerRef = React.useRef<HTMLDivElement>(null);
    const [isTruncated, setIsTruncated] = React.useState(false);

    const checkTruncation = () => {
        const element = containerRef.current;
        if (element) {
            // On cherche un element avec truncate a l'interieur ou on verifie le container lui-meme
            const target = element.querySelector('.truncate') || element;
            setIsTruncated(target.scrollWidth > target.clientWidth);
        }
    };

    React.useEffect(() => {
        checkTruncation();
        // Petit délai pour laisser le layout se stabiliser
        const timer = setTimeout(checkTruncation, 100);
        window.addEventListener('resize', checkTruncation);
        return () => {
            window.removeEventListener('resize', checkTruncation);
            clearTimeout(timer);
        };
    }, [tooltipText, children]);

    return (
        <div className="w-full min-w-0 relative group/tooltip" ref={containerRef}>
            {isTruncated && (
                <div className={cn(
                    "absolute bottom-full mb-2 px-2 py-1 bg-neutral-900 dark:bg-neutral-100 text-white dark:text-neutral-900 text-[11px] rounded pointer-events-none opacity-0 group-hover/tooltip:opacity-100 transition-opacity z-[100] whitespace-nowrap shadow-xl border border-white/10 dark:border-black/10 max-w-[300px] truncate",
                    align === 'right' ? 'right-0' : align === 'center' ? 'left-1/2 -translate-x-1/2' : 'left-0'
                )}>
                    {tooltipText}
                    <div className={cn(
                        "absolute top-full -mt-px border-4 border-transparent border-t-neutral-900 dark:border-t-neutral-100",
                        align === 'right' ? 'right-2' : align === 'center' ? 'left-1/2 -translate-x-1/2' : 'left-2'
                    )} />
                </div>
            )}
            <div className="w-full min-w-0">
                {children}
            </div>
        </div>
    );
};

function Table<T>({
    data,
    columns,
    keyExtractor,
    emptyMessage = "Aucune donnée disponible",
    onRowClick,
    rowClassName,
    onCellUpdate,
    selectedIds,
    onSelectRow,
    onSelectAll,
    isAllSelected,
    virtualized = true,
    rowHeight = 52,
    overscan = 8,
}: TableProps<T>) {
    const [editingCell, setEditingCell] = React.useState<{ rowId: string | number, accessor: keyof T } | null>(null);
    const [editValue, setEditValue] = React.useState<any>("");
    const scrollRef = React.useRef<HTMLDivElement>(null);
    const [scrollTop, setScrollTop] = React.useState(0);
    const [viewportHeight, setViewportHeight] = React.useState(0);
    
    // Calcul de la grille avec support du min-width + colonne selection
    const gridTemplateColumns = `${onSelectRow ? '48px ' : ''}${columns.map(col => {
        if (col.width === '1fr') {
            return `minmax(${col.minWidth || '150px'}, 1fr)`;
        }
        return col.width || '1fr';
    }).join(' ')}`;

    const handleStartEdit = (e: React.MouseEvent, rowId: string | number, col: Column<T>, currentVal: any) => {
        if (!col.editable || !col.accessor || !onCellUpdate) return;
        e.stopPropagation();
        setEditingCell({ rowId, accessor: col.accessor });
        setEditValue(currentVal);
    };

    const handleCommitEdit = (item: T) => {
        if (editingCell && onCellUpdate) {
            onCellUpdate(item, editingCell.accessor, editValue);
        }
        setEditingCell(null);
    };

    const handleKeyDown = (e: React.KeyboardEvent, item: T) => {
        if (e.key === 'Enter') handleCommitEdit(item);
        if (e.key === 'Escape') setEditingCell(null);
    };

    React.useEffect(() => {
        const element = scrollRef.current;
        if (!element) return;

        const updateViewportHeight = () => setViewportHeight(element.clientHeight);
        updateViewportHeight();

        const resizeObserver = new ResizeObserver(updateViewportHeight);
        resizeObserver.observe(element);

        return () => resizeObserver.disconnect();
    }, []);

    React.useEffect(() => {
        setScrollTop(scrollRef.current?.scrollTop || 0);
    }, [data.length]);

    const totalHeight = data.length * rowHeight;
    const shouldVirtualize = virtualized && data.length > 0;
    const visibleRange = React.useMemo(() => {
        if (!shouldVirtualize) {
            return { startIndex: 0, endIndex: data.length };
        }

        const visibleCount = Math.ceil(viewportHeight / rowHeight);
        const startIndex = Math.max(0, Math.floor(scrollTop / rowHeight) - overscan);
        const endIndex = Math.min(data.length, startIndex + visibleCount + overscan * 2);

        return { startIndex, endIndex };
    }, [data.length, overscan, rowHeight, scrollTop, shouldVirtualize, viewportHeight]);

    const renderRow = (item: T, index: number) => {
        const id = keyExtractor(item);
        const isSelected = selectedIds?.has(id);

        return (
            <div
                key={id}
                onClick={() => onRowClick && onRowClick(item)}
                className={cn(
                    "grid items-center hover:bg-gray-100 dark:hover:bg-neutral-800/40 transition-colors group relative border-b border-gray-100 dark:border-neutral-800 last:border-b-0",
                    isSelected && "bg-primary-50/50 dark:bg-primary-900/10",
                    onRowClick && "cursor-pointer",
                    rowClassName && rowClassName(item)
                )}
                style={{
                    gridTemplateColumns,
                    ...(shouldVirtualize
                        ? {
                            height: `${rowHeight}px`,
                            position: 'absolute',
                            left: 0,
                            right: 0,
                            top: 0,
                            transform: `translateY(${index * rowHeight}px)`
                        }
                        : { minHeight: `${rowHeight}px` })
                }}
            >
                {onSelectRow && (
                    <div
                        className="px-4 py-2 flex items-center justify-center"
                        onClick={(e) => e.stopPropagation()}
                    >
                        <input
                            type="checkbox"
                            className="w-4 h-4 rounded border-gray-300 dark:border-neutral-700 text-primary-600 focus:ring-primary-500 cursor-pointer bg-white dark:bg-neutral-900"
                            checked={isSelected}
                            onChange={() => onSelectRow(id)}
                        />
                    </div>
                )}
                {columns.map((col, colIndex) => {
                    const isEditing = editingCell?.rowId === id && editingCell?.accessor === col.accessor;
                    const content = col.render ? col.render(item) : (col.accessor ? (item[col.accessor] as React.ReactNode) : null);
                    const rawValue = col.accessor ? item[col.accessor] : null;

                    let tooltipText = "";
                    if (col.truncate && !isEditing) {
                        if (col.accessor && item[col.accessor]) tooltipText = String(item[col.accessor]);
                        else if (typeof content === 'string') tooltipText = content;
                    }

                    return (
                        <div
                            key={colIndex}
                            className={cn(
                                "px-4 py-2 text-[13px] flex items-center min-w-0 h-full relative overflow-hidden",
                                col.align === 'right' ? 'justify-end text-right' : col.align === 'center' ? 'justify-center text-center' : 'justify-start text-left',
                                col.className,
                                col.editable && "hover:bg-primary-500/5 cursor-text"
                            )}
                            onClick={(e) => col.editable && handleStartEdit(e, id, col, rawValue)}
                        >
                            {isEditing ? (
                                <input
                                    autoFocus
                                    type={col.editType || 'text'}
                                    className="w-full bg-white dark:bg-neutral-900 border border-primary-500 rounded px-2 py-1 outline-none shadow-sm text-[13px]"
                                    value={editValue}
                                    onChange={(e) => setEditValue(e.target.value)}
                                    onBlur={() => handleCommitEdit(item)}
                                    onKeyDown={(e) => handleKeyDown(e, item)}
                                    onClick={(e) => e.stopPropagation()}
                                />
                            ) : col.truncate && tooltipText ? (
                                <TruncatedTooltip tooltipText={tooltipText} align={col.align}>
                                    {content}
                                </TruncatedTooltip>
                            ) : (
                                <div className="w-full min-w-0">
                                    {content}
                                </div>
                            )}
                        </div>
                    );
                })}
            </div>
        );
    };

    return (
        <div
            ref={scrollRef}
            data-no-pull-refresh="true"
            className="h-full min-h-0 w-full overflow-auto overscroll-contain bg-transparent scrollbar-thin"
            onScroll={(event) => setScrollTop(event.currentTarget.scrollTop)}
        >
            <div className="min-h-full min-w-[760px] md:min-w-full">
            {/* Header */}
            <div 
                className="grid items-center bg-gray-50 dark:bg-[#121212] border-b border-black/[0.05] dark:border-white/10 sticky top-0 z-20"
                style={{ gridTemplateColumns }}
            >
                {onSelectRow && (
                    <div className="px-4 py-3 flex items-center justify-center">
                        <input
                            type="checkbox"
                            className="w-4 h-4 rounded border-gray-300 dark:border-neutral-700 text-primary-600 focus:ring-primary-500 cursor-pointer bg-white dark:bg-neutral-900"
                            checked={isAllSelected}
                            onChange={(e) => {
                                e.stopPropagation();
                                onSelectAll?.();
                            }}
                        />
                    </div>
                )}
                {columns.map((col, index) => (
                    <div
                        key={index}
                        className={cn(
                            "px-4 py-3 text-[10px] font-bold text-gray-500 dark:text-neutral-500 uppercase tracking-widest whitespace-nowrap",
                            col.align === 'right' ? 'text-right' : col.align === 'center' ? 'text-center' : 'text-left',
                            col.headerClassName
                        )}
                    >
                        {col.header}
                    </div>
                ))}
            </div>

            {/* Body */}
            <div className="min-h-0">
                {data.length > 0 ? (
                    <div
                        className={cn(shouldVirtualize && "relative")}
                        style={shouldVirtualize ? { height: `${totalHeight}px` } : undefined}
                    >
                        {shouldVirtualize
                            ? data.slice(visibleRange.startIndex, visibleRange.endIndex).map((item, offset) => renderRow(item, visibleRange.startIndex + offset))
                            : data.map((item, index) => renderRow(item, index))
                        }
                    </div>
                ) : (
                    <div className="min-h-[260px] flex flex-col items-center justify-center text-gray-400 p-12">
                        {React.isValidElement(emptyMessage) ? emptyMessage : <span className="text-sm">{emptyMessage}</span>}
                    </div>
                )}
            </div>
            </div>
        </div>
    );
}

export default Table;
