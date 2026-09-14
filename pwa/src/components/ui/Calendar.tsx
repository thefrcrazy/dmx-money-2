import React, { useState } from 'react';
import { ChevronLeft, ChevronRight } from 'lucide-react';
import { format, addMonths, subMonths, startOfMonth, endOfMonth, startOfWeek, endOfWeek, isSameMonth, isSameDay, eachDayOfInterval } from 'date-fns';
import { fr } from 'date-fns/locale';

interface CalendarProps {
    selectedDate: Date;
    onDateSelect: (date: Date) => void;
    onClose?: () => void;
}

const Calendar: React.FC<CalendarProps> = ({ selectedDate, onDateSelect, onClose }) => {
    const [currentMonth, setCurrentMonth] = useState(selectedDate || new Date());

    const nextMonth = () => setCurrentMonth(addMonths(currentMonth, 1));
    const prevMonth = () => setCurrentMonth(subMonths(currentMonth, 1));

    const monthStart = startOfMonth(currentMonth);
    const monthEnd = endOfMonth(monthStart);
    const startDate = startOfWeek(monthStart, { weekStartsOn: 1 });
    const endDate = endOfWeek(monthEnd, { weekStartsOn: 1 });

    const days = eachDayOfInterval({ start: startDate, end: endDate });

    const weekDays = ['Lun', 'Mar', 'Mer', 'Jeu', 'Ven', 'Sam', 'Dim'];

    return (
        <div className="bg-white dark:bg-[#121212] border border-black/[0.05] dark:border-white/10 rounded-xl shadow-2xl p-4 w-[280px] animate-in fade-in zoom-in duration-200">
            {/* Header */}
            <div className="flex items-center justify-between mb-4">
                <button 
                    type="button"
                    onClick={prevMonth}
                    className="p-1.5 rounded-lg hover:bg-gray-100 dark:hover:bg-neutral-800 text-gray-500 transition-colors"
                >
                    <ChevronLeft className="w-4 h-4" />
                </button>
                <div className="text-sm font-bold text-gray-900 dark:text-gray-100 capitalize">
                    {format(currentMonth, 'MMMM yyyy', { locale: fr })}
                </div>
                <button 
                    type="button"
                    onClick={nextMonth}
                    className="p-1.5 rounded-lg hover:bg-gray-100 dark:hover:bg-neutral-800 text-gray-500 transition-colors"
                >
                    <ChevronRight className="w-4 h-4" />
                </button>
            </div>

            {/* Weekdays */}
            <div className="grid grid-cols-7 mb-2">
                {weekDays.map(day => (
                    <div key={day} className="text-center text-[10px] font-bold text-gray-400 uppercase tracking-widest">
                        {day}
                    </div>
                ))}
            </div>

            {/* Days */}
            <div className="grid grid-cols-7 gap-1">
                {days.map((day, idx) => {
                    const isSelected = isSameDay(day, selectedDate);
                    const isCurrentMonth = isSameMonth(day, monthStart);
                    const isToday = isSameDay(day, new Date());

                    return (
                        <button
                            key={idx}
                            type="button"
                            onClick={() => {
                                onDateSelect(day);
                                if (onClose) onClose();
                            }}
                            className={`
                                h-8 w-8 rounded-lg flex items-center justify-center text-xs transition-all
                                ${isSelected 
                                    ? 'bg-primary-500 text-white font-bold shadow-md scale-110' 
                                    : isCurrentMonth
                                        ? 'text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-neutral-800'
                                        : 'text-gray-300 dark:text-gray-600 hover:bg-gray-50 dark:hover:bg-neutral-900/50'
                                }
                                ${isToday && !isSelected ? 'border border-primary-500/50 text-primary-500 font-bold' : ''}
                            `}
                        >
                            {format(day, 'd')}
                        </button>
                    );
                })}
            </div>

            {/* Quick Actions */}
            <div className="mt-4 pt-4 border-t border-black/[0.05] dark:border-white/10 flex justify-between">
                <button
                    type="button"
                    onClick={() => {
                        onDateSelect(new Date());
                        if (onClose) onClose();
                    }}
                    className="text-[10px] font-bold text-primary-600 hover:text-primary-700 uppercase tracking-wider"
                >
                    Aujourd'hui
                </button>
                {onClose && (
                    <button
                        type="button"
                        onClick={onClose}
                        className="text-[10px] font-bold text-gray-400 hover:text-gray-600 uppercase tracking-wider"
                    >
                        Fermer
                    </button>
                )}
            </div>
        </div>
    );
};

export default Calendar;
