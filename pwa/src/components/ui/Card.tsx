import React from 'react';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

interface CardProps extends React.HTMLAttributes<HTMLDivElement> {
    title?: string;
    subtitle?: string;
    icon?: React.ElementType;
    action?: React.ReactNode;
    noPadding?: boolean;
}

const Card: React.FC<CardProps> = ({
    children,
    className,
    title,
    subtitle,
    icon: Icon,
    action,
    noPadding = false,
    ...props
}) => {
    const hasHeader = Boolean(title || action);

    return (
        <div
            className={cn(
                "app-card overflow-hidden",
                className
            )}
            {...props}
        >
            {hasHeader && (
                // Sur mobile, en-tête de section iOS : titre en gras, sans filet sous le titre.
                <div className="flex items-center justify-between gap-3 px-4 pt-3.5 pb-1.5 md:px-6 md:py-4 md:border-b border-black/[0.04] dark:border-white/[0.06]">
                    <div className="flex min-w-0 items-center gap-2.5">
                        {Icon && <Icon className="h-5 w-5 shrink-0 text-primary-500" />}
                        <div className="min-w-0">
                            {title && <h3 className="truncate text-[17px] font-semibold leading-tight tracking-tight text-gray-950 dark:text-white md:text-sm md:font-extrabold md:leading-none">{title}</h3>}
                            {subtitle && <p className="mt-0.5 text-[13px] text-[var(--color-text-secondary)] md:mt-1 md:text-[10px] md:text-gray-400">{subtitle}</p>}
                        </div>
                    </div>
                    {action && <div className="shrink-0">{action}</div>}
                </div>
            )}
            <div className={cn(!noPadding && (hasHeader ? "px-4 pb-4 pt-2 md:p-6" : "p-4 md:p-6"))}>
                {children}
            </div>
        </div>
    );
};

export default Card;
