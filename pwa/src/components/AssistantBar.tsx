import React, { useState } from 'react';
import { Sparkles, ArrowUp, AlertCircle, Loader2 } from 'lucide-react';
import Card from './ui/Card';
import { dbService, type AssistantAnswer } from '../services/db';

const EXAMPLES = [
    'ajoute 12,50 € en alimentation',
    'quel est mon solde ?',
    'combien me reste-t-il en carburant ?',
    'prochaines échéances',
];

/**
 * Assistant du compagnon mobile.
 *
 * La phrase est envoyée au pont local du Mac : l'application de bureau la fait normaliser par son
 * modèle sur l'appareil quand il est disponible, puis le noyau l'interprète et calcule la réponse.
 * Le mobile n'affiche que ce que le Mac renvoie.
 */
const AssistantBar: React.FC = () => {
    const [text, setText] = useState('');
    const [answer, setAnswer] = useState<AssistantAnswer | null>(null);
    const [error, setError] = useState<string | null>(null);
    const [isAsking, setIsAsking] = useState(false);

    const ask = async (value: string) => {
        const question = value.trim();
        if (!question || isAsking) return;
        setIsAsking(true);
        setError(null);
        try {
            const result = await dbService.askAssistant(question);
            setAnswer(result);
            setText('');
            // Les écritures remontent par la surveillance de `dataVersion` du compagnon : la
            // liste se recharge d'elle-même au tick suivant.
        } catch (requestError) {
            setError(requestError instanceof Error ? requestError.message : 'Demande impossible.');
            setAnswer(null);
        } finally {
            setIsAsking(false);
        }
    };

    return (
        <Card title="Assistant" icon={Sparkles} subtitle="Dictez une opération ou posez une question">
            {/* Champ de message, bouton d'envoi rond intégré comme dans Messages. */}
            <form
                className="relative"
                onSubmit={event => {
                    event.preventDefault();
                    void ask(text);
                }}
            >
                <input
                    type="text"
                    value={text}
                    onChange={event => setText(event.target.value)}
                    placeholder="Ajoute 12,50 € en alimentation"
                    enterKeyHint="send"
                    disabled={isAsking}
                    aria-label="Demande à l'assistant"
                    className="app-input h-11 w-full !pr-12 text-sm"
                />
                <button
                    type="submit"
                    disabled={isAsking || !text.trim()}
                    className="absolute right-1.5 top-1/2 flex h-8 w-8 -translate-y-1/2 items-center justify-center rounded-full bg-primary-500 text-white transition-opacity disabled:opacity-30"
                    aria-label="Envoyer"
                >
                    {isAsking ? <Loader2 className="h-4 w-4 animate-spin" /> : <ArrowUp className="h-4 w-4" strokeWidth={2.5} />}
                </button>
            </form>

            {!answer && !error && (
                <div className="-mx-4 mt-3 flex gap-2 overflow-x-auto px-4 scrollbar-hide md:mx-0 md:flex-wrap md:px-0" data-no-pull-refresh="true">
                    {EXAMPLES.map(example => (
                        <button
                            key={example}
                            type="button"
                            onClick={() => void ask(example)}
                            className="shrink-0 rounded-full bg-primary-500/10 px-3 py-1.5 text-[13px] font-medium text-primary-600 dark:text-primary-400"
                        >
                            {example}
                        </button>
                    ))}
                </div>
            )}

            {error && (
                <div className="mt-3 flex items-start gap-2 text-sm text-red-600 dark:text-red-400">
                    <AlertCircle size={16} className="mt-0.5 shrink-0" />
                    <span>{error}</span>
                </div>
            )}

            {answer && (
                <div className="mt-3 space-y-1">
                    <p className="text-sm font-semibold">{answer.summary}</p>
                    {answer.details.map(detail => (
                        <p key={detail} className="text-xs text-gray-600 dark:text-gray-300">{detail}</p>
                    ))}
                    {answer.understood && answer.interpreted && (
                        <p className="text-[11px] text-gray-400">Compris comme : {answer.interpreted}</p>
                    )}
                </div>
            )}
        </Card>
    );
};

export default AssistantBar;
