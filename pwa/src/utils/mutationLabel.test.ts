import { expect, test } from 'bun:test';
import { mutationLabel } from './mutationLabel';

test('sync issue labels tolerate legacy malformed, null and non-object bodies', () => {
    for (const body of ['null', '{invalid', '[]', '123', '{"description":{}}']) {
        expect(mutationLabel({ path: '/api/transactions', body })).toBe('Modification');
    }
    expect(mutationLabel({ path: '/api/transactions', body: '{"description":"Restaurant fictif"}' })).toBe('Restaurant fictif');
    expect(mutationLabel({ path: '/api/settings', body: 'null' })).toBe('Paramètres');
});
