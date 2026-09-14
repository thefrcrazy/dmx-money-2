import React from 'react';

interface SkeletonProps extends React.HTMLAttributes<HTMLDivElement> {
    className?: string;
}

const Skeleton: React.FC<SkeletonProps> = ({ className, ...props }) => {
    return (
        <div
            className={`animate-pulse rounded-md bg-gray-200/80 dark:bg-neutral-800/80 ${className}`}
            {...props}
        />
    );
};

export default Skeleton;