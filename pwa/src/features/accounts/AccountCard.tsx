import React from 'react';
import { Edit2, Trash2, Tag } from 'lucide-react';
import { Account, Transaction } from '../../types';
import Button from '../../components/ui/Button';
import { ICONS } from '../../constants/icons';

interface AccountCardProps {
    account: Account;
    transactions: Transaction[];
    onEdit: (account: Account) => void;
    onDelete: (id: string) => void;
    dragHandleProps?: any;
    isDragging?: boolean;
    isDragOverlay?: boolean;
}

const AccountCard: React.FC<AccountCardProps> = ({
    account,
    transactions,
    onEdit,
    onDelete,
    dragHandleProps,
    isDragging,
    isDragOverlay
}) => {
    const currentBalance = account.initialBalance + transactions
        .filter(t => t.accountId === account.id)
        .reduce((sum, t) => sum + (t.type === 'income' ? t.amount : -t.amount), 0);

    const clearedBalance = account.initialBalance + transactions
        .filter(t => t.accountId === account.id && t.checked)
        .reduce((sum, t) => sum + (t.type === 'income' ? t.amount : -t.amount), 0);

    const Icon = ICONS[account.icon] || Tag;

    return (
        <div
            className={`app-card p-6 hover:shadow-md transition-all app-account-card ${isDragging ? 'opacity-50' : ''} ${isDragOverlay ? 'shadow-xl scale-105 cursor-grabbing' : 'cursor-grab active:cursor-grabbing'}`}
            {...dragHandleProps}
        >
            <div className="flex items-start justify-between mb-4 app-account-header">
                <div className="flex items-center gap-3 app-account-info">
                    <div
                        className="p-3 rounded-xl app-account-icon-container"
                        style={{ backgroundColor: `${account.color}20`, color: account.color }}
                    >
                        <Icon className="w-6 h-6 app-account-icon" />
                    </div>
                    <div>
                        <h3 className="font-semibold text-gray-900 dark:text-gray-200 app-account-name">{account.name}</h3>
                        <p className="text-sm text-gray-500 dark:text-gray-400 app-account-type">{account.type}</p>
                    </div>
                </div>
                <div className="flex gap-1 app-account-actions" onPointerDown={(e) => e.stopPropagation()}>
                    <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => onEdit(account)}
                        className="text-gray-400 hover:text-primary-600 dark:text-gray-500 dark:hover:text-primary-400 app-account-action-edit"
                        title="Modifier"
                        icon={Edit2}
                    />
                    <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => onDelete(account.id)}
                        className="text-gray-400 hover:text-red-600 dark:text-gray-500 dark:hover:text-red-400 app-account-action-delete"
                        title="Supprimer"
                        icon={Trash2}
                    />
                </div>
            </div>
            <div className="space-y-1 app-account-balance-container">
                <p className="text-sm text-gray-500 dark:text-gray-400 app-balance-label">Solde actuel</p>
                <p className={`text-2xl font-bold ${currentBalance >= 0 ? 'text-gray-900 dark:text-gray-200' : 'text-red-600 dark:text-red-500'} app-balance-value`}>
                    {new Intl.NumberFormat('fr-FR', { minimumFractionDigits: 2, maximumFractionDigits: 2 }).format(currentBalance)} €
                </p>
                <div className="flex items-center justify-between pt-2 mt-2 border-t border-gray-100 dark:border-neutral-800 app-cleared-balance-container">
                    <p className="text-xs text-gray-400 dark:text-gray-500 app-cleared-label">Solde pointé</p>
                    <p className="text-sm font-medium text-gray-600 dark:text-gray-400 app-cleared-value">
                        {new Intl.NumberFormat('fr-FR', { minimumFractionDigits: 2, maximumFractionDigits: 2 }).format(clearedBalance)} €
                    </p>
                </div>
            </div>
        </div>
    );
};

export default AccountCard;
