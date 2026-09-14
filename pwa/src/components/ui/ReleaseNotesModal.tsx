import React from 'react';
import { createPortal } from 'react-dom';
import { CalendarDays, Check, Sparkles, X } from 'lucide-react';
import { CHANGELOG, VersionUpdate } from '../../constants/changelog';
import { ICONS } from '../../constants/icons';
import Button from './Button';

interface ReleaseNotesModalProps {
    isOpen: boolean;
    onClose: () => void;
    versionData?: VersionUpdate;
}

const ReleaseNotesModal: React.FC<ReleaseNotesModalProps> = ({
    isOpen,
    onClose,
    versionData = CHANGELOG[0]
}) => {
    if (!isOpen) return null;

    return createPortal(
        <div className="fixed inset-0 z-[120] flex items-end justify-center bg-slate-950/35 p-0 backdrop-blur-sm animate-in fade-in duration-200 sm:items-center sm:p-4">
            <div
                className="flex max-h-[calc(100dvh-env(safe-area-inset-top))] w-full flex-col overflow-hidden rounded-b-none rounded-t-2xl border border-primary-100/70 border-x-0 border-b-0 bg-white shadow-2xl animate-in slide-in-from-bottom-6 duration-200 dark:border-white/10 dark:bg-neutral-950 sm:max-h-[82vh] sm:max-w-xl sm:rounded-2xl sm:border"
                role="dialog"
                aria-modal="true"
                aria-labelledby="release-notes-title"
            >
                <div className="sticky top-0 z-10 border-b border-primary-100/80 bg-white px-4 py-3 dark:border-white/10 dark:bg-neutral-950 sm:px-5">
                    <div className="flex items-start justify-between gap-3">
                        <div className="min-w-0">
                            <div className="mb-1.5 flex flex-wrap items-center gap-2">
                                <span className="inline-flex items-center gap-1.5 rounded-full bg-primary-500/10 px-2 py-0.5 text-[10px] font-bold uppercase tracking-wider text-primary-700 dark:text-primary-300">
                                    <Sparkles className="h-3 w-3" />
                                    Nouveautés
                                </span>
                                <span className="inline-flex items-center gap-1 rounded-full bg-sky-50 px-2 py-0.5 text-[10px] font-semibold text-sky-700 dark:bg-sky-500/10 dark:text-sky-300">
                                    <CalendarDays className="h-3 w-3" />
                                    {versionData.date}
                                </span>
                            </div>
                            <div className="flex flex-wrap items-center gap-2">
                                <h2 id="release-notes-title" className="min-w-0 text-lg font-bold leading-snug text-gray-950 dark:text-white sm:truncate sm:text-xl">
                                    {versionData.title}
                                </h2>
                                <span className="shrink-0 rounded-md bg-primary-50 px-1.5 py-0.5 text-[11px] font-semibold text-primary-600 dark:bg-primary-500/10 dark:text-primary-300">
                                    v{versionData.version}
                                </span>
                            </div>
                        </div>
                        <button
                            onClick={onClose}
                            className="shrink-0 rounded-lg p-2 text-gray-400 transition-colors hover:bg-primary-50 hover:text-primary-700 dark:hover:bg-neutral-900 dark:hover:text-gray-200"
                            aria-label="Fermer le changelog"
                        >
                            <X className="h-4 w-4" />
                        </button>
                    </div>
                </div>

                <div className="flex-1 overflow-y-auto bg-gradient-to-b from-primary-50/35 via-white to-white px-4 py-4 custom-scrollbar dark:from-primary-950/15 dark:via-neutral-950 dark:to-neutral-950 sm:px-5">
                    {versionData.features && versionData.features.length > 0 && (
                        <div className="mb-5 grid grid-cols-1 gap-2 sm:grid-cols-2">
                            {versionData.features.map((feature, idx) => {
                                const IconComp = ICONS[feature.icon] || Sparkles;
                                return (
                                    <div key={idx} className="flex min-w-0 gap-3 rounded-xl border border-primary-100/80 bg-white p-3 shadow-sm shadow-primary-950/5 dark:border-white/10 dark:bg-neutral-900/80 dark:shadow-black/20">
                                        <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-primary-500/10 text-primary-600 dark:bg-primary-500/15 dark:text-primary-300">
                                            <IconComp className="h-4 w-4" />
                                        </div>
                                        <div className="min-w-0">
                                            <h3 className="text-sm font-semibold leading-snug text-gray-950 dark:text-white">{feature.title}</h3>
                                            <p className="mt-0.5 text-xs leading-relaxed text-gray-600 dark:text-neutral-400">
                                                {feature.description}
                                            </p>
                                        </div>
                                    </div>
                                );
                            })}
                        </div>
                    )}

                    <div className="overflow-hidden rounded-xl border border-primary-100/80 bg-white shadow-sm shadow-primary-950/5 dark:border-white/10 dark:bg-neutral-900/80 dark:shadow-black/20">
                        <div className="flex items-center justify-between border-b border-primary-100/80 bg-primary-50/80 px-3 py-2 dark:border-white/10 dark:bg-neutral-900">
                            <h3 className="text-[11px] font-bold uppercase tracking-wider text-primary-700 dark:text-primary-300">
                                Changements
                            </h3>
                            <span className="text-[11px] font-semibold text-primary-500 dark:text-primary-300">
                                {versionData.changes.length}
                            </span>
                        </div>
                        <div className="divide-y divide-primary-100/70 dark:divide-white/10">
                            {versionData.changes.map((change, idx) => (
                                <div key={idx} className="grid grid-cols-[1.75rem_1fr] gap-2 px-3 py-2.5 sm:grid-cols-[2rem_1fr]">
                                    <div className="flex h-5 w-5 items-center justify-center rounded-full bg-emerald-500/10 text-emerald-600 dark:text-emerald-400">
                                        <Check className="h-3 w-3" />
                                    </div>
                                    <p className="text-sm leading-relaxed text-gray-700 dark:text-neutral-300">
                                        {change}
                                    </p>
                                </div>
                            ))}
                        </div>
                    </div>
                </div>

                <div className="border-t border-primary-100/80 bg-white px-4 py-3 pb-[max(0.75rem,env(safe-area-inset-bottom))] dark:border-white/10 dark:bg-neutral-950 sm:px-5 sm:pb-3">
                    <Button
                        onClick={onClose}
                        fullWidth
                        size="md"
                    >
                        Fermer
                    </Button>
                </div>
            </div>
        </div>,
        document.body
    );
};

export default ReleaseNotesModal;
