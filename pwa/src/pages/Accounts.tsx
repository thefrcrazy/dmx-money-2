import React, { useMemo, useState } from 'react';
import { Plus, Settings as SettingsIcon, Trash2, Edit2, Search } from 'lucide-react';
import {
    DndContext,
    closestCenter,
    KeyboardSensor,
    PointerSensor,
    useSensor,
    useSensors,
    DragOverlay,
    defaultDropAnimationSideEffects,
    DragStartEvent,
    DragOverEvent,
    DragEndEvent
} from '@dnd-kit/core';
import {
    arrayMove,
    SortableContext,
    sortableKeyboardCoordinates,
    verticalListSortingStrategy
} from '@dnd-kit/sortable';
import { useBank } from '../context/BankContext';
import { useSettings } from '../context/SettingsContext';
import { Account } from '../types';
import FormPopup from '../components/ui/FormPopup';
import ConfirmModal from '../components/ui/ConfirmModal';
import Button from '../components/ui/Button';
import Input from '../components/ui/Input';
import ColorPicker from '../components/ui/ColorPicker';
import SearchableSelect from '../components/ui/SearchableSelect';
import MultiSelect from '../components/ui/MultiSelect';
import SortableGroupItem from '../features/accounts/SortableGroupItem';
import AccountCard from '../features/accounts/AccountCard';
import { COLORS } from '../constants/icons';
import { isMobileCompanion } from '../utils/runtime';

const normalizeSearchValue = (value: unknown) => String(value ?? '')
    .toLowerCase()
    .normalize('NFD')
    .replace(/[\u0300-\u036f]/g, '');

const dropAnimation = {
    sideEffects: defaultDropAnimationSideEffects({
        styles: {
            active: {
                opacity: '0.5',
            },
        },
    }),
};

const ACCOUNT_TYPES = [
    { id: 'Courant', label: 'Courant' },
    { id: 'Épargne', label: 'Épargne' },
    { id: 'Investissement', label: 'Investissement' },
    { id: 'Espèces', label: 'Espèces' },
];

const Accounts: React.FC = () => {
    const { accounts: allAccounts, addAccount, updateAccount, deleteAccount, transactions } = useBank();
    const { settings, updateAccountGroup, updateCustomGroups, renameCustomGroup, updateCustomGroupsOrder, updateAccountsOrder } = useSettings();

    const [isModalOpen, setIsModalOpen] = useState(false);
    const [isGroupModalOpen, setIsGroupModalOpen] = useState(false);
    const [editingAccount, setEditingAccount] = useState<Account | null>(null);
    const [searchTerm, setSearchTerm] = useState('');
    const [filterTypes, setFilterTypes] = useState<string[]>([]);
    const [deleteConfirmation, setDeleteConfirmation] = useState<{ isOpen: boolean; accountId: string | null }>({
        isOpen: false,
        accountId: null
    });

    const [formData, setFormData] = useState({
        name: '',
        type: 'Courant',
        initialBalance: '',
        icon: 'Wallet',
        color: '#3b82f6',
        group: ''
    });

    // Group Management State
    const [newGroupName, setNewGroupName] = useState('');
    const [editingGroup, setEditingGroup] = useState<{ oldName: string; newName: string } | null>(null);
    const [deleteGroupConfirmation, setDeleteGroupConfirmation] = useState<{ isOpen: boolean; groupName: string | null }>({
        isOpen: false,
        groupName: null
    });

    const [activeId, setActiveId] = useState<string | null>(null);
    const [activeType, setActiveType] = useState<'group' | 'account' | null>(null);
    const mobileMode = isMobileCompanion();

    const accounts = useMemo(() => {
        const searchTokens = normalizeSearchValue(searchTerm).split(/\s+/).filter(Boolean);

        return allAccounts.filter(account => {
            if (filterTypes.length > 0 && !filterTypes.includes(account.type)) return false;
            if (searchTokens.length === 0) return true;

            const groupName = settings.accountGroups?.[account.id] || 'Non groupés';
            const accountBalance = account.initialBalance + transactions
                .filter(transaction => transaction.accountId === account.id)
                .reduce((sum, transaction) => sum + (transaction.type === 'income' ? transaction.amount : -transaction.amount), 0);
            const searchableText = [
                account.name,
                account.type,
                account.initialBalance,
                accountBalance,
                account.color,
                account.icon,
                groupName
            ].map(normalizeSearchValue).join(' ');

            return searchTokens.every(token => searchableText.includes(token));
        });
    }, [allAccounts, filterTypes, searchTerm, settings.accountGroups, transactions]);

    const sensors = useSensors(
        useSensor(PointerSensor, {
            activationConstraint: mobileMode
                ? {
                    delay: 250,
                    tolerance: 8,
                }
                : {
                    distance: 8,
                },
        }),
        useSensor(KeyboardSensor, {
            coordinateGetter: sortableKeyboardCoordinates,
        })
    );

    const handleOpenModal = (account?: Account) => {
        if (account) {
            setEditingAccount(account);
            setFormData({
                name: account.name,
                type: account.type,
                initialBalance: account.initialBalance.toString(),
                icon: account.icon || 'Wallet',
                color: account.color || '#3b82f6',
                group: settings.accountGroups?.[account.id] || ''
            });
        } else {
            setEditingAccount(null);
            setFormData({
                name: '',
                type: 'Courant',
                initialBalance: '',
                icon: 'Wallet',
                color: '#3b82f6',
                group: ''
            });
        }
        setIsModalOpen(true);
    };

    const handleSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        const accountData = {
            name: formData.name,
            type: formData.type,
            initialBalance: parseFloat(formData.initialBalance) || 0,
            icon: formData.icon,
            color: formData.color
        };

        if (editingAccount) {
            await updateAccount({ ...editingAccount, ...accountData });
            await updateAccountGroup(editingAccount.id, formData.group);
        } else {
            const newAccountId = await addAccount(accountData);
            if (formData.group) {
                await updateAccountGroup(newAccountId, formData.group);
            }
        }
        setIsModalOpen(false);
    };

    const handleDelete = (id: string) => {
        setDeleteConfirmation({ isOpen: true, accountId: id });
    };

    const confirmDelete = () => {
        if (deleteConfirmation.accountId) {
            deleteAccount(deleteConfirmation.accountId);
            setDeleteConfirmation({ isOpen: false, accountId: null });
        }
    };

    const handleAddGroup = () => {
        if (newGroupName && !settings.customGroups?.includes(newGroupName)) {
            updateCustomGroups([...(settings.customGroups || []), newGroupName]);
            setNewGroupName('');
        }
    };

    const handleDeleteGroup = (groupName: string) => {
        setDeleteGroupConfirmation({ isOpen: true, groupName });
    };

    const confirmDeleteGroup = async () => {
        const groupName = deleteGroupConfirmation.groupName;
        if (groupName) {
            const accountsInGroup = allAccounts.filter(acc => settings.accountGroups?.[acc.id] === groupName);
            for (const acc of accountsInGroup) {
                await updateAccountGroup(acc.id, '');
            }
            updateCustomGroups((settings.customGroups || []).filter(g => g !== groupName));
            setDeleteGroupConfirmation({ isOpen: false, groupName: null });
        }
    };

    const handleStartRename = (groupName: string) => {
        setEditingGroup({ oldName: groupName, newName: groupName });
    };

    const handleSaveRename = () => {
        if (editingGroup && editingGroup.newName && editingGroup.newName !== editingGroup.oldName) {
            if (!settings.customGroups?.includes(editingGroup.newName)) {
                renameCustomGroup(editingGroup.oldName, editingGroup.newName);
            }
        }
        setEditingGroup(null);
    };

    const handleTypeChange = (type: string) => {
        let newIcon = formData.icon;
        let newColor = formData.color;

        switch (type) {
            case 'Épargne':
                newIcon = 'PiggyBank';
                newColor = '#10b981';
                break;
            case 'Espèces':
                newIcon = 'Banknote';
                newColor = '#f59e0b';
                break;
            case 'Investissement':
                newIcon = 'TrendingUp';
                newColor = '#8b5cf6';
                break;
            default:
                newIcon = 'Wallet';
                newColor = '#3b82f6';
                break;
        }
        setFormData({ ...formData, type, icon: newIcon, color: newColor });
    };

    // Group accounts logic
    const groupedAccounts = React.useMemo(() => {
        const groups: Record<string, Account[]> = {};
        const groupOrder = settings.customGroupsOrder || settings.customGroups || [];

        groupOrder.forEach(g => groups[g] = []);
        groups['Non groupés'] = [];

        (settings.customGroups || []).forEach(g => {
            if (!groups[g]) groups[g] = [];
        });

        const sortedAccounts = [...accounts].sort((a, b) => {
            const order = settings.accountsOrder || [];
            const indexA = order.indexOf(a.id);
            const indexB = order.indexOf(b.id);
            if (indexA === -1 && indexB === -1) return 0;
            if (indexA === -1) return 1;
            if (indexB === -1) return -1;
            return indexA - indexB;
        });

        sortedAccounts.forEach(acc => {
            const groupName = settings.accountGroups?.[acc.id];
            if (groupName && (groups[groupName])) {
                groups[groupName].push(acc);
            } else {
                groups['Non groupés'].push(acc);
            }
        });

        return groups;
    }, [accounts, settings.accountGroups, settings.customGroups, settings.customGroupsOrder, settings.accountsOrder]);

    const orderedGroups = React.useMemo(() => {
        const groups = Object.keys(groupedAccounts);
        // Ensure 'Non groupés' is always last if it exists
        if (groups.includes('Non groupés')) {
            return [...groups.filter(g => g !== 'Non groupés'), 'Non groupés'];
        }
        return groups;
    }, [groupedAccounts]);

    // DnD Handlers
    const handleDragStart = (event: DragStartEvent) => {
        const { active } = event;
        const activeData = active.data.current;

        if (activeData) {
            setActiveId(active.id as string);
            setActiveType(activeData.type);
        }
    };

    const handleDragOver = (event: DragOverEvent) => {
        const { active, over } = event;
        if (!over) return;

        const activeId = active.id;
        const overId = over.id;

        if (activeId === overId) return;

        const isActiveAccount = active.data.current?.type === 'account';
        const isOverAccount = over.data.current?.type === 'account';
        const isOverGroup = over.data.current?.type === 'group';

        if (!isActiveAccount) return;

        // Moving account over another account or group
        if (isActiveAccount && (isOverAccount || isOverGroup)) {
            const activeAccount = accounts.find(a => a.id === activeId);
            if (!activeAccount) return;

            let targetGroupName = '';

            if (isOverGroup) {
                targetGroupName = overId as string;
            } else if (isOverAccount) {
                const overAccount = accounts.find(a => a.id === overId);
                if (overAccount) {
                    targetGroupName = settings.accountGroups?.[overAccount.id] || 'Non groupés';
                }
            }

            const currentGroupName = settings.accountGroups?.[activeId] || 'Non groupés';

            if (targetGroupName !== currentGroupName && targetGroupName !== 'Non groupés') {
                // Visual feedback only, actual update happens on drag end
            }
        }
    };

    const handleDragEnd = async (event: DragEndEvent) => {
        const { active, over } = event;
        setActiveId(null);
        setActiveType(null);

        if (!over) return;

        const activeId = active.id as string;
        const overId = over.id as string;

        if (activeId === overId) return;

        const activeType = active.data.current?.type;
        const overType = over.data.current?.type;

        // Reordering Groups
        if (activeType === 'group' && overType === 'group') {
            const currentOrder = settings.customGroupsOrder || settings.customGroups || [];
            const oldIndex = currentOrder.indexOf(activeId);
            const newIndex = currentOrder.indexOf(overId);

            if (oldIndex !== -1 && newIndex !== -1) {
                const newOrder = arrayMove(currentOrder, oldIndex, newIndex);
                await updateCustomGroupsOrder(newOrder);
            }
            return;
        }

        // Reordering Accounts
        if (activeType === 'account') {
            const activeAccount = accounts.find(a => a.id === activeId);
            if (!activeAccount) return;

            // Dropping on another account
            if (overType === 'account') {
                const overAccount = accounts.find(a => a.id === overId);
                if (!overAccount) return;

                const currentOrder = settings.accountsOrder || accounts.map(a => a.id);
                const oldIndex = currentOrder.indexOf(activeId);
                const newIndex = currentOrder.indexOf(overId);

                // Update order
                if (oldIndex !== -1 && newIndex !== -1) {
                    const newOrder = arrayMove(currentOrder, oldIndex, newIndex);
                    await updateAccountsOrder(newOrder);
                }

                // Update group if different
                const targetGroup = settings.accountGroups?.[overId] || '';
                const currentGroup = settings.accountGroups?.[activeId] || '';

                if (targetGroup !== currentGroup) {
                    await updateAccountGroup(activeId, targetGroup);
                }
            }
            // Dropping on a group header
            else if (overType === 'group') {
                const targetGroup = overId === 'Non groupés' ? '' : overId;
                const currentGroup = settings.accountGroups?.[activeId] || '';

                if (targetGroup !== currentGroup) {
                    await updateAccountGroup(activeId, targetGroup);
                }
            }
        }
    };

    const groupOptions = [
        { id: '', label: 'Aucun groupe' },
        ...(settings.customGroups || []).map(g => ({ id: g, label: g }))
    ];

    return (
        <div className="space-y-6">
            <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3 w-full">
                <h2 className="hidden md:block text-2xl font-bold text-gray-900 dark:text-gray-200">Mes Comptes</h2>
                <div className="flex items-center gap-2 w-full sm:w-auto">
                    <Button onClick={() => setIsGroupModalOpen(true)} variant="secondary" size="sm" icon={SettingsIcon} className="flex-1 sm:flex-none">
                        Gérer les groupes
                    </Button>
                    <Button onClick={() => handleOpenModal()} size="sm" icon={Plus} className="flex-1 sm:flex-none">
                        Nouveau compte
                    </Button>
                </div>
            </div>

            <div className="flex flex-col lg:flex-row gap-3 px-1">
                <div className="w-full lg:max-w-sm">
                    <Input
                        placeholder="Rechercher un compte..."
                        value={searchTerm}
                        onChange={(event) => setSearchTerm(event.target.value)}
                        icon={Search}
                    />
                </div>
                <MultiSelect
                    value={filterTypes}
                    onChange={setFilterTypes}
                    options={ACCOUNT_TYPES}
                    placeholder="Tous les types"
                    className="w-full sm:w-56"
                />
            </div>

            {accounts.length > 0 ? (
                <DndContext
                    sensors={sensors}
                    collisionDetection={closestCenter}
                    onDragStart={handleDragStart}
                    onDragOver={handleDragOver}
                    onDragEnd={handleDragEnd}
                >
                    <div className="space-y-8">
                        <SortableContext
                            items={orderedGroups.filter(g => g !== 'Non groupés')}
                            strategy={verticalListSortingStrategy}
                        >
                            {orderedGroups.map(groupName => {
                                const groupAccounts = groupedAccounts[groupName] || [];
                                if (groupAccounts.length === 0 && groupName === 'Non groupés' && !settings.customGroups?.includes(groupName)) return null;

                                return (
                                    <SortableGroupItem
                                        key={groupName}
                                        groupName={groupName}
                                        accounts={groupAccounts}
                                        transactions={transactions}
                                        onEditAccount={handleOpenModal}
                                        onDeleteAccount={handleDelete}
                                    />
                                );
                            })}
                        </SortableContext>
                    </div>

                    <DragOverlay dropAnimation={dropAnimation}>
                        {activeId && activeType === 'account' ? (
                            <AccountCard
                                account={accounts.find(a => a.id === activeId)!}
                                transactions={transactions}
                                onEdit={() => { }}
                                onDelete={() => { }}
                                isDragOverlay
                            />
                        ) : activeId && activeType === 'group' ? (
                            <div className="bg-white dark:bg-[#121212] p-4 rounded-xl shadow-xl border border-indigo-500/50">
                                <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-200 flex items-center gap-2">
                                    <span className="text-sm font-normal text-gray-500 dark:text-gray-400 bg-gray-100 dark:bg-neutral-800 px-2 py-0.5 rounded-full">
                                        {groupedAccounts[activeId]?.length || 0}
                                    </span>
                                    {activeId}
                                </h3>
                            </div>
                        ) : null}
                    </DragOverlay>
                </DndContext>
            ) : (
                <div className="text-center py-12 app-card border-dashed">
                    {allAccounts.length === 0 ? (
                        <>
                            <SettingsIcon className="w-12 h-12 text-gray-300 dark:text-gray-600 mx-auto mb-3" />
                            <p className="text-gray-500 dark:text-gray-400">Aucun compte configuré</p>
                            <Button
                                variant="ghost"
                                onClick={() => handleOpenModal()}
                                className="mt-2"
                            >
                                Créer votre premier compte
                            </Button>
                        </>
                    ) : (
                        <>
                            <Search className="w-12 h-12 text-gray-300 dark:text-gray-600 mx-auto mb-3" />
                            <p className="text-gray-500 dark:text-gray-400">Aucun compte ne correspond aux filtres.</p>
                        </>
                    )}
                </div>
            )}

            {/* Modals */}
            <FormPopup
                isOpen={isModalOpen}
                onClose={() => setIsModalOpen(false)}
                title={editingAccount ? "Modifier le compte" : "Nouveau compte"}
                onSubmit={handleSubmit}
                submitLabel={editingAccount ? "Mettre à jour" : "Créer"}
            >
                <div className="space-y-4">
                    <div>
                        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                            Nom du compte
                        </label>
                        <Input
                            type="text"
                            required
                            value={formData.name}
                            onChange={(e) => setFormData({ ...formData, name: e.target.value })}
                            placeholder="Ex: Compte Courant"
                        />
                    </div>

                    <div className="grid grid-cols-2 gap-4">
                        <div>
                            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                                Type
                            </label>
                            <SearchableSelect
                                value={formData.type}
                                onChange={handleTypeChange}
                                options={ACCOUNT_TYPES}
                                placeholder="Sélectionner un type"
                            />
                        </div>
                        <div>
                            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                                Solde initial
                            </label>
                            <Input
                                type="number"
                                step="0.01"
                                required
                                value={formData.initialBalance}
                                onChange={(e) => setFormData({ ...formData, initialBalance: e.target.value })}
                                placeholder="0.00"
                            />
                        </div>
                    </div>

                    <div>
                        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                            Groupe
                        </label>
                        <SearchableSelect
                            value={formData.group}
                            onChange={(value) => setFormData({ ...formData, group: value })}
                            options={groupOptions}
                            placeholder="Sélectionner un groupe"
                        />
                    </div>

                    <div>
                        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Couleur</label>
                        <ColorPicker
                            value={formData.color}
                            onChange={(color) => setFormData({ ...formData, color })}
                            colors={COLORS}
                            size="md"
                        />
                    </div>
                </div>
            </FormPopup>

            <ConfirmModal
                isOpen={deleteConfirmation.isOpen}
                onClose={() => setDeleteConfirmation({ isOpen: false, accountId: null })}
                onConfirm={confirmDelete}
                title="Supprimer le compte"
                message="Êtes-vous sûr de vouloir supprimer ce compte ? Cette action est irréversible et supprimera toutes les transactions associées."
            />

            {/* Group Management Modal */}
            <FormPopup
                isOpen={isGroupModalOpen}
                onClose={() => setIsGroupModalOpen(false)}
                title="Gérer les groupes"
                onSubmit={(e) => { e.preventDefault(); setIsGroupModalOpen(false); }}
                submitLabel="Fermer"
            >
                <div className="space-y-6">
                    <div className="flex gap-2">
                        <Input
                            type="text"
                            value={newGroupName}
                            onChange={(e) => setNewGroupName(e.target.value)}
                            containerClassName="flex-1"
                            placeholder="Nouveau groupe..."
                            onKeyDown={(e) => {
                                if (e.key === 'Enter') {
                                    e.preventDefault();
                                    handleAddGroup();
                                }
                            }}
                        />
                        <Button onClick={handleAddGroup} icon={Plus}>
                            Ajouter
                        </Button>
                    </div>

                    <div className="space-y-2">
                        {(settings.customGroups || []).map(group => (
                            <div key={group} className="flex items-center justify-between p-3 bg-gray-100 dark:bg-neutral-700 rounded-lg group hover:bg-gray-200 dark:hover:bg-gray-600 transition-colors cursor-pointer">
                                {editingGroup?.oldName === group ? (
                                    <div className="flex items-center gap-2 flex-1 mr-2">
                                        <Input
                                            type="text"
                                            value={editingGroup.newName}
                                            onChange={(e) => setEditingGroup({ ...editingGroup, newName: e.target.value })}
                                            className="h-8 text-sm"
                                            autoFocus
                                            onBlur={handleSaveRename}
                                            onKeyDown={(e) => {
                                                if (e.key === 'Enter') handleSaveRename();
                                                if (e.key === 'Escape') setEditingGroup(null);
                                            }}
                                        />
                                    </div>
                                ) : (
                                    <span className="font-medium text-gray-900 dark:text-gray-200">{group}</span>
                                )}
                                <div className="flex gap-1">
                                    <Button
                                        variant="ghost"
                                        size="sm"
                                        onClick={() => handleStartRename(group)}
                                        icon={Edit2}
                                        className="text-gray-400 hover:text-primary-600 dark:text-gray-500 dark:hover:text-primary-400"
                                        title="Modifier"
                                    />
                                    <Button
                                        variant="ghost"
                                        size="sm"
                                        onClick={() => handleDeleteGroup(group)}
                                        icon={Trash2}
                                        className="text-gray-400 hover:text-red-600 dark:text-gray-500 dark:hover:text-red-400"
                                        title="Supprimer"
                                    />
                                </div>
                            </div>
                        ))}
                        {(settings.customGroups || []).length === 0 && (
                            <p className="text-center text-gray-500 dark:text-gray-400 py-4">
                                Aucun groupe personnalisé
                            </p>
                        )}
                    </div>
                </div>
            </FormPopup>

            <ConfirmModal
                isOpen={deleteGroupConfirmation.isOpen}
                onClose={() => setDeleteGroupConfirmation({ isOpen: false, groupName: null })}
                onConfirm={confirmDeleteGroup}
                title="Supprimer le groupe"
                message={`Êtes-vous sûr de vouloir supprimer le groupe "${deleteGroupConfirmation.groupName}" ? Les comptes de ce groupe ne seront pas supprimés mais seront déplacés dans "Non groupés".`}
            />
        </div>
    );
};

export default Accounts;
