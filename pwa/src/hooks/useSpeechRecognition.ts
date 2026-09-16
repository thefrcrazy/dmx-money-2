import { useState, useEffect, useRef, useCallback } from "react";

export interface UseSpeechRecognitionOptions {
  onResult?: (transcript: string, isFinal: boolean) => void;
  onEnd?: (finalTranscript: string) => void;
  lang?: string;
}

interface SpeechResult { isFinal: boolean; 0: { transcript: string } }
interface SpeechEvent { results: ArrayLike<SpeechResult> }
interface Recognition {
  lang: string;
  continuous: boolean;
  interimResults: boolean;
  maxAlternatives: number;
  onstart: (() => void) | null;
  onresult: ((event: SpeechEvent) => void) | null;
  onerror: ((event: { error: string }) => void) | null;
  onend: (() => void) | null;
  start(): void;
  stop(): void;
  abort(): void;
}
type SpeechWindow = Window & {
  SpeechRecognition?: new () => Recognition;
  webkitSpeechRecognition?: new () => Recognition;
};

// Results contains the entire session; resultIndex only marks the first changed result.
export function collectSpeechResults(results: ArrayLike<SpeechResult>) {
  const items = Array.from(results);
  return {
    transcript: items.map(item => item[0].transcript.trim()).filter(Boolean).join(" "),
    finalTranscript: items.filter(item => item.isFinal).map(item => item[0].transcript.trim()).filter(Boolean).join(" "),
    isFinal: items.length > 0 && items.every(item => item.isFinal),
  };
}

export function useSpeechRecognition({ onResult, onEnd, lang = "fr-FR" }: UseSpeechRecognitionOptions = {}) {
  const [isListening, setIsListening] = useState(false);
  const [transcript, setTranscript] = useState("");
  const [error, setError] = useState<string | null>(null);
  const recognitionRef = useRef<Recognition | null>(null);
  const callbacks = useRef({ onResult, onEnd });
  callbacks.current = { onResult, onEnd };
  const ctor = typeof window === "undefined" ? undefined :
    ((window as SpeechWindow).SpeechRecognition ?? (window as SpeechWindow).webkitSpeechRecognition);
  const isSupported = Boolean(ctor);

  const cancelListening = useCallback(() => {
    const recognition = recognitionRef.current;
    recognitionRef.current = null;
    if (recognition) {
      recognition.onstart = recognition.onresult = recognition.onerror = recognition.onend = null;
      try { recognition.abort(); } catch { /* Already ended. */ }
    }
    setIsListening(false);
  }, []);

  const stopListening = useCallback(() => {
    try { recognitionRef.current?.stop(); } catch { cancelListening(); }
  }, [cancelListening]);

  const startListening = useCallback(() => {
    if (!ctor) {
      setError("La reconnaissance vocale n'est pas disponible sur ce navigateur.");
      return;
    }
    cancelListening();
    setError(null);
    setTranscript("");
    const recognition = new ctor();
    recognitionRef.current = recognition;
    recognition.lang = lang;
    recognition.continuous = false;
    recognition.interimResults = true;
    recognition.maxAlternatives = 1;
    let finalTranscript = "";
    let failed = false;
    const current = () => recognitionRef.current === recognition;
    recognition.onstart = () => { if (current()) setIsListening(true); };
    recognition.onresult = event => {
      if (!current()) return;
      const result = collectSpeechResults(event.results);
      finalTranscript = result.finalTranscript;
      setTranscript(result.transcript);
      callbacks.current.onResult?.(result.transcript, result.isFinal);
    };
    recognition.onerror = event => {
      if (!current()) return;
      failed = true;
      const messages: Record<string, string> = {
        'not-allowed': "Accès au micro refusé. Autorisez-le dans les réglages du navigateur.",
        'service-not-allowed': "Le service de dictée n'est pas autorisé sur ce navigateur.",
        'no-speech': "Aucune parole reconnue. Réessayez en parlant près du microphone.",
        'audio-capture': "Microphone indisponible. Vérifiez le périphérique sélectionné.",
        'network': "Le service de transcription est injoignable. Vérifiez la connexion.",
        'language-not-supported': "Cette langue de dictée n'est pas prise en charge.",
      };
      if (event.error !== 'aborted') setError(messages[event.error] ?? "La transcription a échoué. Vous pouvez saisir votre demande.");
      setIsListening(false);
    };
    recognition.onend = () => {
      if (!current()) return;
      recognitionRef.current = null;
      setIsListening(false);
      if (!failed) callbacks.current.onEnd?.(finalTranscript);
    };
    try {
      setIsListening(true);
      recognition.start();
    } catch {
      cancelListening();
      setError("Impossible de démarrer la dictée. Réessayez ou saisissez votre demande.");
    }
  }, [ctor, lang, cancelListening]);

  useEffect(() => () => {
    const recognition = recognitionRef.current;
    recognitionRef.current = null;
    if (recognition) {
      recognition.onstart = recognition.onresult = recognition.onerror = recognition.onend = null;
      try { recognition.abort(); } catch { /* Already ended. */ }
    }
  }, []);

  return { isListening, transcript, error, isSupported, startListening, stopListening, cancelListening };
}
