import { expect, test } from 'bun:test';
import { formatCurrency } from './format';

test('money matches the French desktop format regardless of browser language', () => {
    expect(formatCurrency(1234.56)).toBe('1\u202f234,56\u00a0€');
    expect(formatCurrency(-12.5)).toBe('-12,50\u00a0€');
    expect(formatCurrency(0)).toBe('0,00\u00a0€');
});
