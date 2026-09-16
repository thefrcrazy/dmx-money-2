import React, { useState } from "react";
import { Sparkles, ArrowUp, AlertCircle, Loader2, Mic, MicOff } from "lucide-react";
import Card from "./ui/Card";
import { dbService, type AssistantAnswer } from "../services/db";
import { useSpeechRecognition } from "../hooks/useSpeechRecognition";

const EXAMPLES = [
  "ajoute 12,50 € en alimentation",
  "quel est mon solde ?",
  "combien me reste-t-il en carburant ?",
  "prochaines échéances",
];

/**
 * Assistant du compagnon mobile.
 *
 * La phrase est envoyée au pont local du Mac : l'application de bureau la fait normaliser par son
 * modèle sur l'appareil quand il est disponible, puis le noyau l'interprète et calcule la réponse.
 * Le mobile n'affiche que ce que le Mac renvoie.
 */
const AssistantBar: React.FC = () => {
  const [text, setText] = useState("");
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
      setText("");
    } catch (requestError) {
      setError(requestError instanceof Error ? requestError.message : "Demande impossible.");
      setAnswer(null);
    } finally {
      setIsAsking(false);
    }
  };

  const {
    isListening,
    error: speechError,
    isSupported: isSpeechSupported,
    startListening,
    stopListening,
  } = useSpeechRecognition({
    onResult: (transcript) => {
      setText(transcript);
    },
    onEnd: (finalTranscript) => {
      const trimmed = finalTranscript.trim();
      if (trimmed) {
        setText(trimmed);
        void ask(trimmed);
      }
    },
  });

  const toggleListening = () => {
    if (isListening) {
      stopListening();
    } else {
      startListening();
    }
  };

  return (
    <Card title="Assistant" icon={Sparkles} subtitle="Dictez une opération ou posez une question">
      {/* Champ de message avec bouton micro et bouton d'envoi. */}
      <form
        className="relative flex items-center"
        onSubmit={event => {
          event.preventDefault();
          if (isListening) stopListening();
          void ask(text);
        }}
      >
        <input
          type="text"
          value={text}
          onChange={event => setText(event.target.value)}
          placeholder={isListening ? "Parlez maintenant..." : "Ajoute 12,50 € en alimentation"}
          enterKeyHint="send"
          disabled={isAsking}
          aria-label="Demande à l'assistant"
          className={`app-input h-11 w-full !pr-20 text-sm transition-all ${
            isListening ? "border-red-500 ring-2 ring-red-500/20" : ""
          }`}
        />
        <div className="absolute right-1.5 flex items-center gap-1">
          {isSpeechSupported && (
            <button
              type="button"
              onClick={toggleListening}
              disabled={isAsking}
              className={`flex h-8 w-8 items-center justify-center rounded-full transition-all ${
                isListening
                  ? "bg-red-500 text-white animate-pulse"
                  : "bg-[var(--ios-fill-tertiary)] text-[var(--ios-label)] hover:bg-[var(--ios-fill-secondary)] active:scale-95"
              }`}
              aria-label={isListening ? "Arrêter l'écoute" : "Dicter vocalement"}
            >
              {isListening ? <MicOff className="h-4 w-4" /> : <Mic className="h-4 w-4" />}
            </button>
          )}
          <button
            type="submit"
            disabled={isAsking || !text.trim()}
            className="flex h-8 w-8 items-center justify-center rounded-full bg-primary-500 text-white transition-opacity disabled:opacity-30 active:scale-95"
            aria-label="Envoyer"
          >
            {isAsking ? <Loader2 className="h-4 w-4 animate-spin" /> : <ArrowUp className="h-4 w-4" strokeWidth={2.5} />}
          </button>
        </div>
      </form>

      {isListening && (
        <p className="mt-2 text-xs text-red-500 animate-pulse font-medium">
          À l'écoute... Le message sera envoyé dès la fin de votre phrase.
        </p>
      )}

      {speechError && (
        <p className="mt-2 text-xs text-amber-600 dark:text-amber-400">
          {speechError}
        </p>
      )}

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
