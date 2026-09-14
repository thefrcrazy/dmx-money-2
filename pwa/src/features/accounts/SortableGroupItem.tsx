import React from 'react';
import { useSortable } from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import { SortableContext, rectSortingStrategy } from '@dnd-kit/sortable';
import { GripVertical } from 'lucide-react';
import { Account, Transaction } from '../../types';
import SortableAccountItem from './SortableAccountItem';

interface SortableGroupItemProps {
    groupName: string;
    accounts: Account[];
    transactions: Transaction[];
    onEditAccount: (account: Account) => void;
    onDeleteAccount: (id: string) => void;
}

const SortableGroupItem: React.FC<SortableGroupItemProps> = ({
    groupName,
    accounts,
    transactions,
    onEditAccount,
    onDeleteAccount
}) => {
    const {
        attributes,
        listeners,
        setNodeRef,
        transform,
        transition,
        isDragging,
        over
    } = useSortable({
        id: groupName,
        data: {
            type: 'group',
            groupName
        },
        disabled: groupName === 'Non groupés'
    });

    const style = {
        transform: CSS.Transform.toString(transform),
        transition,
        zIndex: isDragging ? 5 : undefined,
    };

    const isOver = over?.id === groupName;

    return (
        <div
            ref={setNodeRef}
            style={style}
            className={`transition-all rounded-xl p-2 -m-2 app-account-group ${isOver && !isDragging ? 'bg-indigo-50 dark:bg-indigo-900/20 ring-2 ring-indigo-500' : ''} ${isDragging ? 'opacity-50' : ''}`}
        >
            <h3
                className="text-lg font-semibold text-gray-900 dark:text-gray-200 mb-4 flex items-center gap-2 app-group-header"
                {...attributes}
                {...listeners}
            >
                {groupName !== 'Non groupés' && (
                    <GripVertical className="w-4 h-4 text-gray-400 cursor-grab active:cursor-grabbing app-group-drag-handle" />
                )}
                <span className="app-group-name">{groupName}</span>
                <span className="text-sm font-normal text-gray-500 dark:text-gray-400 bg-gray-100 dark:bg-neutral-800 px-2 py-0.5 rounded-full app-group-count">
                    {accounts.length}
                </span>
            </h3>
            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6 app-group-items">
                <SortableContext items={accounts.map(a => a.id)} strategy={rectSortingStrategy}>
                    {accounts.map(acc => (
                        <SortableAccountItem
                            key={acc.id}
                            account={acc}
                            transactions={transactions}
                            onEdit={onEditAccount}
                            onDelete={onDeleteAccount}
                        />
                    ))}
                </SortableContext>
            </div>
        </div>
    );
};

export default SortableGroupItem;
