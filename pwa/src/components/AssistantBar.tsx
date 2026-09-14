import React, { useState } from 'react';
import { Sparkles, CornerDownLeft, AlertCircle } from 'lucide-react';
import Card from './ui/Card';
import Input from './ui/Input';
import Button from './ui/Button';
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
            <div className="flex items-center gap-2">
                <Input
                    value={text}
                    onChange={event => setText(event.target.value)}
                    onKeyDown={event => {
                        if (event.key === 'Enter') void ask(text);
                    }}
                    placeholder="ajoute 12,50 € en alimentation"
                    containerClassName="flex-1"
                    disabled={isAsking}
                />
                <Button onClick={() => void ask(text)} disabled={isAsking || !text.trim()} icon={CornerDownLeft}>
                    {isAsking ? 'Un instant…' : 'Envoyer'}
                </Button>
            </div>

            {!answer && !error && (
                <div className="mt-3 flex flex-wrap gap-2">
                    {EXAMPLES.map(example => (
                        <button
                            key={example}
                            type="button"
                            onClick={() => void ask(example)}
                            className="rounded-full border border-gray-200 dark:border-gray-700 px-3 py-1 text-xs text-gray-600 dark:text-gray-300"
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
