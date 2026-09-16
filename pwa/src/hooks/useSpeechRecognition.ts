import { useState, useEffect, useRef, useCallback } from "react";

export interface UseSpeechRecognitionOptions {
  onResult?: (transcript: string, isFinal: boolean) => void;
  onEnd?: (finalTranscript: string) => void;
  lang?: string;
}

export function useSpeechRecognition({
  onResult,
  onEnd,
  lang = "fr-FR",
}: UseSpeechRecognitionOptions = {}) {
  const [isListening, setIsListening] = useState(false);
  const [transcript, setTranscript] = useState("");
  const [error, setError] = useState<string | null>(null);
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const recognitionRef = useRef<any>(null);

  const isSupported =
    typeof window !== "undefined" &&
    ("SpeechRecognition" in window || "webkitSpeechRecognition" in window);

  const stopListening = useCallback(() => {
    if (recognitionRef.current) {
      try {
        recognitionRef.current.stop();
      } catch {
        // Ignorer l'erreur d'arrêt si déjà arrêté
      }
    }
    setIsListening(false);
  }, []);

  const startListening = useCallback(() => {
    if (!isSupported) {
      setError("La reconnaissance vocale n'est pas supportée sur ce navigateur.");
      return;
    }

    setError(null);
    setTranscript("");

    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const SpeechRecognitionClass =
      (window as any).SpeechRecognition || (window as any).webkitSpeechRecognition;

    try {
      if (recognitionRef.current) {
        try {
          recognitionRef.current.abort();
        } catch {
          // Ignorer
        }
      }

      const recognition = new SpeechRecognitionClass();
      recognition.lang = lang;
      recognition.continuous = false;
      recognition.interimResults = true;
      recognition.maxAlternatives = 1;

      let lastTranscript = "";

      recognition.onstart = () => {
        setIsListening(true);
      };

      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      recognition.onresult = (event: any) => {
        let current = "";
        let isFinal = false;

        for (let i = event.resultIndex; i < event.results.length; i++) {
          const item = event.results[i];
          current += item[0].transcript;
          if (item.isFinal) {
            isFinal = true;
          }
        }

        lastTranscript = current;
        setTranscript(current);
        onResult?.(current, isFinal);
      };

      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      recognition.onerror = (event: any) => {
        console.warn("Erreur reconnaissance vocale :", event.error);
        if (event.error !== "no-speech") {
          setError(
            event.error === "not-allowed"
              ? "Accès au micro refusé. Autorisez le micro dans les réglages du navigateur."
              : "Erreur de reconnaissance vocale."
          );
        }
        setIsListening(false);
      };

      recognition.onend = () => {
        setIsListening(false);
        onEnd?.(lastTranscript);
      };

      recognitionRef.current = recognition;
      recognition.start();
    } catch (err) {
      console.error("Échec du démarrage de la reconnaissance vocale :", err);
      setError("Impossible d'activer le microphone.");
      setIsListening(false);
    }
  }, [isSupported, lang, onResult, onEnd]);

  useEffect(() => {
    return () => {
      if (recognitionRef.current) {
        try {
          recognitionRef.current.abort();
        } catch {
          // Ignorer
        }
      }
    };
  }, []);

  return {
    isListening,
    transcript,
    error,
    isSupported,
    startListening,
    stopListening,
  };
}
