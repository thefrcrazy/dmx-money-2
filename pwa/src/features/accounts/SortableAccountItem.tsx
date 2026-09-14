import React from 'react';
import { useSortable } from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import { Account, Transaction } from '../../types';
import AccountCard from './AccountCard';

interface SortableAccountItemProps {
    account: Account;
    transactions: Transaction[];
    onEdit: (account: Account) => void;
    onDelete: (id: string) => void;
}

const SortableAccountItem: React.FC<SortableAccountItemProps> = ({
    account,
    transactions,
    onEdit,
    onDelete
}) => {
    const {
        attributes,
        listeners,
        setNodeRef,
        transform,
        transition,
        isDragging
    } = useSortable({
        id: account.id,
        data: {
            type: 'account',
            account
        }
    });

    const style = {
        transform: CSS.Transform.toString(transform),
        transition,
        zIndex: isDragging ? 10 : undefined,
    };

    return (
        <div ref={setNodeRef} style={style} {...attributes} {...listeners} className="app-sortable-account-item">
            <AccountCard
                account={account}
                transactions={transactions}
                onEdit={onEdit}
                onDelete={onDelete}
                isDragging={isDragging}
            />
        </div>
    );
};

export default SortableAccountItem;
