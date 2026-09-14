import React, { useMemo } from 'react';
import { PieChart, Pie, Cell, ResponsiveContainer, Tooltip, Legend } from 'recharts';

interface CategoryData {
    id: string;
    name: string;
    value: number;
    color: string;
    [key: string]: any;
}

interface CategoryPieChartProps {
    title: string;
    data: CategoryData[];
    hiddenCategories: string[];
    onToggle: (id: string) => void;
    emptyMessage?: string;
}

const CustomTooltip = ({ active, payload }: any) => {
    if (active && payload && payload.length && payload[0].value > 0) {
        return (
            <div className="app-card p-3 shadow-lg app-chart-tooltip">
                <p className="font-medium text-gray-900 dark:text-gray-200 mb-2 app-tooltip-label">{payload[0].name}</p>
                <p className="text-sm app-tooltip-value" style={{ color: payload[0].payload.fill }}>
                    {new Intl.NumberFormat('fr-FR', { style: 'currency', currency: 'EUR' }).format(payload[0].value)}
                </p>
            </div>
        );
    }
    return null;
};

const CategoryPieChart: React.FC<CategoryPieChartProps> = React.memo(({
    title,
    data,
    hiddenCategories,
    onToggle,
    emptyMessage = "Aucune donnée à afficher"
}) => {
    const chartData = useMemo(() =>
        data.map(d => hiddenCategories.includes(d.id) ? { ...d, value: 0, color: '#9ca3af' } : d),
        [data, hiddenCategories]
    );

    const hasVisibleData = useMemo(() => chartData.some(d => d.value > 0), [chartData]);

    const handleToggle = (entry: any) => {
        // Robust ID retrieval
        const id = entry.id || entry.payload?.id;
        if (id) {
            onToggle(id);
        }
    };

    // Prepare legend payload from the FULL data, not just visible data
    const legendPayload = useMemo(() => data.map(item => {
        const isHidden = hiddenCategories.includes(item.id);
        return {
            id: item.id,
            type: 'square',
            value: item.name,
            color: isHidden ? '#9ca3af' : item.color,
            isHidden // Custom property for formatter
        };
    }), [data, hiddenCategories]);

    return (
        <div className="app-card p-6 app-chart-card">
            <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-200 mb-4 app-chart-title">{title}</h3>
            <div className="h-80 min-w-0 app-chart-container" style={{ minHeight: '320px' }}>

                {hasVisibleData ? (
                    <ResponsiveContainer width="100%" height="100%" minWidth={0}>
                        <PieChart>
                            <Pie
                                data={chartData}
                                cx="50%"
                                cy="50%"
                                innerRadius={60}
                                outerRadius={100}
                                paddingAngle={5}
                                cornerRadius={6}
                                dataKey="value"
                            >
                                {chartData.map((entry) => (
                                    <Cell key={entry.id} fill={entry.color} stroke="none" className="app-chart-segment outline-none focus:outline-none" />
                                ))}
                            </Pie>
                            <Tooltip content={<CustomTooltip />} />
                            <Legend
                                key={hiddenCategories.join(',')}
                                onClick={handleToggle}
                                wrapperStyle={{ cursor: 'pointer' }}
                                // @ts-ignore
                                payload={legendPayload}
                                formatter={(value, entry: any) => {
                                    // Use the prop directly for maximum reliability
                                    // Check both direct id and payload.id (Recharts behavior varies)
                                    const id = entry.id || entry.payload?.id;
                                    const isHidden = id ? hiddenCategories.includes(id) : false;

                                    return (
                                        <span style={{
                                            color: isHidden ? '#9ca3af' : 'var(--color-text-primary)',
                                            textDecoration: isHidden ? 'line-through' : 'none'
                                        }}>
                                            {value}
                                        </span>
                                    );
                                }}
                            />
                        </PieChart>
                    </ResponsiveContainer>
                ) : (
                    <div className="h-full flex items-center justify-center text-gray-500">
                        {emptyMessage}
                    </div>
                )}
            </div>
        </div>
    );
});

export default CategoryPieChart;
