import { describe, expect, test } from 'bun:test';
import { collectSpeechResults } from './useSpeechRecognition';

describe('dictation result aggregation', () => {
  test('keeps earlier final fragments when the next fragment changes', () => {
    const result = collectSpeechResults([
      { isFinal: true, 0: { transcript: 'Virement de 50 euros' } },
      { isFinal: false, 0: { transcript: ' de Courant vers Livret A' } },
    ]);
    expect(result.transcript).toBe('Virement de 50 euros de Courant vers Livret A');
    expect(result.finalTranscript).toBe('Virement de 50 euros');
    expect(result.isFinal).toBe(false);
  });
  test('replaces provisional words instead of duplicating them', () => {
    expect(collectSpeechResults([{ isFinal: false, 0: { transcript: 'ajoute 15' } }]).transcript).toBe('ajoute 15');
    const final = collectSpeechResults([{ isFinal: true, 0: { transcript: 'ajoute 50 euros' } }]);
    expect(final.finalTranscript).toBe('ajoute 50 euros');
    expect(final.isFinal).toBe(true);
    expect(collectSpeechResults([]).isFinal).toBe(false);
  });
});
