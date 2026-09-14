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
    return (
        <div 
            className={cn(
                "app-card overflow-hidden",
                className
            )} 
            {...props}
        >
            {(title || action) && (
                <div className="px-4 py-3.5 md:px-6 md:py-4 border-b border-black/[0.04] dark:border-white/[0.06] flex items-center justify-between">
                    <div className="flex items-center gap-2">
                        {Icon && <Icon className="w-5 h-5 text-primary-500" />}
                        <div>
                            {title && <h3 className="text-[15px] md:text-sm font-extrabold tracking-tight text-gray-950 dark:text-white leading-none">{title}</h3>}
                            {subtitle && <p className="text-[10px] text-gray-400 mt-1">{subtitle}</p>}
                        </div>
                    </div>
                    {action && <div>{action}</div>}
                </div>
            )}
            <div className={cn(!noPadding && "p-4 md:p-6")}>
                {children}
            </div>
        </div>
    );
};

export default Card;
