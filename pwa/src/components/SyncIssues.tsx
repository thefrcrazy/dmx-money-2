import { useEffect, useState } from 'react';
import { dbService } from '../services/db';
import { offlineStore, type OfflineMutation } from '../services/offlineStore';
import FormPopup from './ui/FormPopup';
import ConfirmModal from './ui/ConfirmModal';
import Button from './ui/Button';
import { mutationLabel } from '../utils/mutationLabel';

export default function SyncIssues() {
    const [issues, setIssues] = useState<OfflineMutation[]>([]);
    const [open, setOpen] = useState(false);
    const [confirm, setConfirm] = useState(false);
    const [pending, setPending] = useState(0);
    const [busy, setBusy] = useState(false);
    const [error, setError] = useState<string | null>(null);
    useEffect(() => {
        let active = true;
        const update = async () => {
            try {
                const queue = await offlineStore.listMutations();
                if (active) { setIssues(queue.filter(item => item.failure)); setPending(queue.length); setError(null); }
            } catch (error) {
                const ids = await offlineStore.listMutationIds().catch(() => []);
                if (active) { setPending(ids.length); setError(error instanceof Error ? error.message : 'File hors ligne illisible.'); }
            }
        };
        void update();
        window.addEventListener('dmxmoney-sync-issues', update);
        return () => { active = false; window.removeEventListener('dmxmoney-sync-issues', update); };
    }, []);
    const perform = async (action: () => Promise<void>) => {
        setBusy(true); setError(null);
        try { await action(); }
        catch (error) { setError(error instanceof Error ? error.message : 'Synchronisation indisponible. Vos modifications restent conservées.'); }
        finally { setBusy(false); }
    };
    if (!issues.length && !error) return null;
    return <>
        <div role="status" className="m-4 rounded-xl border border-amber-400 bg-amber-50 p-3 text-sm text-amber-950 dark:bg-amber-950 dark:text-amber-100">
            {issues.length ? `${issues.length} modification(s) nécessitent votre attention. Les changements sont conservés sur ce téléphone.` : error}
            <button className="ml-3 font-semibold underline" onClick={() => setOpen(true)}>Examiner</button>
        </div>
        <FormPopup isOpen={open} onClose={() => { if (!busy) setOpen(false); }} title="Modifications non synchronisées" isSubmitting={busy}>
            <div className="space-y-4 p-4">
                <p className="text-sm">Les modifications indépendantes continuent à se synchroniser. Les changements qui dépendent d’un élément rejeté attendent sa résolution.</p>
                {error && <p role="alert" className="text-red-600">{error}</p>}
                <ul className="space-y-3">{issues.map(item => <li key={item.id} className="rounded-lg border border-black/10 p-3 dark:border-white/10">
                    <p className="font-semibold">{mutationLabel(item)} — {new Date(item.createdAt).toLocaleString('fr-FR')}</p>
                    <p className="text-sm">{item.failure?.message}</p>
                    {!!item.failure?.conflicts?.length && <p className="text-sm">Champs concernés : {item.failure.conflicts.join(', ')}</p>}
                    <Button disabled={busy} variant="secondary" onClick={() => void perform(() => dbService.retrySyncIssue(item.id))}>Réessayer l’envoi</Button>
                </li>)}</ul>
                <p className="text-sm">Pour conserver les valeurs de l’ordinateur, vous pouvez annuler les {pending} modifications encore en attente sur ce téléphone. Le rechargement exige une connexion à l’ordinateur.</p>
                <Button disabled={busy} variant="danger" onClick={() => setConfirm(true)}>Annuler les modifications en attente</Button>
            </div>
        </FormPopup>
        <ConfirmModal isOpen={confirm} onClose={() => setConfirm(false)} title="Reprendre les données de l’ordinateur ?"
            message={`Les ${pending} modifications non synchronisées de ce téléphone seront annulées après vérification de la connexion. Cette action ne modifie pas les données de l’ordinateur.`}
            isDangerous confirmLabel="Annuler et recharger" onConfirm={() => void perform(() => dbService.reloadServerAndDiscardPending())} />
    </>;
}
