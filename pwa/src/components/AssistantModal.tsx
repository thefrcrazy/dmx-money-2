import React, { useState, useEffect, useRef } from "react";
import { Sparkles, ArrowUp, AlertCircle, Loader2, Mic, MicOff } from "lucide-react";
import FormPopup from "./ui/FormPopup";
import { dbService, type AssistantAnswer } from "../services/db";
import { useSpeechRecognition } from "../hooks/useSpeechRecognition";

const EXAMPLES = [
  "Ajoute 12,50 € en alimentation",
  "Quel est mon solde ?",
  "Combien me reste-t-il en carburant ?",
  "Prochaines échéances",
  "Dépenses du mois",
];

interface AssistantModalProps {
  isOpen: boolean;
  onClose: () => void;
}

export const AssistantModal: React.FC<AssistantModalProps> = ({ isOpen, onClose }) => {
  const [text, setText] = useState("");
  const [answer, setAnswer] = useState<AssistantAnswer | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isAsking, setIsAsking] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

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

  useEffect(() => {
    if (isOpen) {
      setTimeout(() => {
        inputRef.current?.focus();
      }, 300);
    } else {
      if (isListening) {
        stopListening();
      }
      setAnswer(null);
      setError(null);
      setText("");
    }
  }, [isOpen]);

  const toggleListening = () => {
    if (isListening) {
      stopListening();
    } else {
      startListening();
    }
  };

  return (
    <FormPopup
      isOpen={isOpen}
      onClose={onClose}
      title="Assistant"
      maxWidth="md"
    >
      <div className="space-y-4 pt-1">
        <p className="text-xs text-[var(--ios-secondary-label)]">
          Dictez ou écrivez une opération, une question de solde ou de budget.
        </p>

        {/* Barre de saisie avec micro et bouton d'envoi */}
        <form
          className="relative flex items-center"
          onSubmit={(event) => {
            event.preventDefault();
            if (isListening) stopListening();
            void ask(text);
          }}
        >
          <input
            ref={inputRef}
            type="text"
            value={text}
            onChange={(event) => setText(event.target.value)}
            placeholder={isListening ? "Parlez maintenant..." : "Ex: Ajoute 12,50 € en alimentation"}
            enterKeyHint="send"
            disabled={isAsking}
            aria-label="Demande à l'assistant"
            className={`app-input h-12 w-full !pr-24 text-sm transition-all ${
              isListening ? "border-red-500 ring-2 ring-red-500/20" : ""
            }`}
          />

          <div className="absolute right-1.5 flex items-center gap-1">
            {isSpeechSupported && (
              <button
                type="button"
                onClick={toggleListening}
                disabled={isAsking}
                className={`flex h-9 w-9 items-center justify-center rounded-full transition-all ${
                  isListening
                    ? "bg-red-500 text-white animate-pulse shadow-md shadow-red-500/30"
                    : "bg-[var(--ios-fill-tertiary)] text-[var(--ios-label)] hover:bg-[var(--ios-fill-secondary)] active:scale-95"
                }`}
                aria-label={isListening ? "Arrêter l'enregistrement" : "Dicter vocalement"}
                title={isListening ? "Arrêter" : "Dicter"}
              >
                {isListening ? <MicOff className="h-4 w-4" /> : <Mic className="h-4 w-4" />}
              </button>
            )}

            <button
              type="submit"
              disabled={isAsking || !text.trim()}
              className="flex h-9 w-9 items-center justify-center rounded-full bg-primary-500 text-white transition-opacity disabled:opacity-30 active:scale-95"
              aria-label="Envoyer"
            >
              {isAsking ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <ArrowUp className="h-4 w-4" strokeWidth={2.5} />
              )}
            </button>
          </div>
        </form>

        {isListening && (
          <div className="flex items-center gap-2 px-1 text-xs font-medium text-red-500 animate-pulse">
            <span className="relative flex h-2 w-2">
              <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-red-400 opacity-75"></span>
              <span className="relative inline-flex rounded-full h-2 w-2 bg-red-500"></span>
            </span>
            <span>À l'écoute... Le message sera envoyé dès que vous aurez fini de parler.</span>
          </div>
        )}

        {speechError && (
          <p className="text-xs text-amber-600 dark:text-amber-400 px-1">
            {speechError}
          </p>
        )}

        {/* Suggestions pré-faites */}
        <div>
          <div className="text-[11px] font-semibold tracking-wider uppercase text-[var(--ios-secondary-label)] mb-2 px-1">
            Suggestions rapides
          </div>
          <div className="flex flex-wrap gap-2">
            {EXAMPLES.map((example) => (
              <button
                key={example}
                type="button"
                onClick={() => void ask(example)}
                disabled={isAsking}
                className="rounded-full bg-primary-500/10 px-3 py-1.5 text-xs font-medium text-primary-600 dark:text-primary-400 hover:bg-primary-500/20 active:scale-95 transition-all text-left"
              >
                {example}
              </button>
            ))}
          </div>
        </div>

        {/* Message d'erreur éventuel */}
        {error && (
          <div className="flex items-start gap-2 rounded-xl bg-red-500/10 p-3 text-sm text-red-600 dark:text-red-400">
            <AlertCircle size={16} className="mt-0.5 shrink-0" />
            <span>{error}</span>
          </div>
        )}

        {/* Réponse de l'assistant */}
        {answer && (
          <div className="rounded-2xl border border-primary-500/20 bg-primary-500/5 p-4 space-y-2">
            <div className="flex items-center gap-2 text-primary-600 dark:text-primary-400 font-semibold text-sm">
              <Sparkles className="h-4 w-4" />
              <span>{answer.summary}</span>
            </div>
            {answer.details.length > 0 && (
              <ul className="space-y-1 pl-6 list-disc text-xs text-[var(--ios-label)]">
                {answer.details.map((detail, idx) => (
                  <li key={idx}>{detail}</li>
                ))}
              </ul>
            )}
            {answer.understood && answer.interpreted && (
              <p className="text-[11px] text-[var(--ios-secondary-label)] pt-1 border-t border-primary-500/10">
                Compris comme : <span className="italic">{answer.interpreted}</span>
              </p>
            )}
          </div>
        )}
      </div>
    </FormPopup>
  );
};

export default AssistantModal;
