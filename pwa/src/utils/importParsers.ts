import { v5 as uuidv5 } from 'uuid';
import type { Transaction } from '../types';

export type ParsedStatementTransaction = {
    date: string;
    amount: number;
    description: string;
    category?: string;
    bankSource?: string;
    bankTransactionId?: string;
};

export type ImportTransactionInput = {
    date: string;
    amount: number;
    type: 'income' | 'expense';
    description: string;
    category: string;
    accountId?: string;
    checked?: boolean;
    bankSource?: string;
    bankTransactionId?: string;
};

const decodeEntities = (value: string) => value
    .replace(/&amp;/g, '&')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'");

const normalizeText = (value: string) => value.trim().replace(/\s+/g, ' ');

export const parseDelimitedRows = (content: string, separator: string, hasHeader = false): string[][] => {
    if (separator.length !== 1) {
        throw new Error('Le séparateur CSV doit contenir un seul caractère.');
    }

    const rows: string[][] = [];
    let row: string[] = [];
    let cell = '';
    let inQuotes = false;
    const input = content.replace(/^\uFEFF/, '');

    const pushCell = () => {
        row.push(cell.trim());
        cell = '';
    };

    const pushRow = () => {
        pushCell();
        if (row.some(value => value.length > 0)) {
            rows.push(row);
        }
        row = [];
    };

    for (let i = 0; i < input.length; i++) {
        const char = input[i];

        if (char === '"') {
            if (inQuotes && input[i + 1] === '"') {
                cell += '"';
                i++;
            } else {
                inQuotes = !inQuotes;
            }
            continue;
        }

        if (char === separator && !inQuotes) {
            pushCell();
            continue;
        }

        if ((char === '\n' || char === '\r') && !inQuotes) {
            if (char === '\r' && input[i + 1] === '\n') i++;
            pushRow();
            continue;
        }

        cell += char;
    }

    if (inQuotes) throw new Error('Le fichier CSV contient un champ entre guillemets non terminé.');
    if (cell.length > 0 || row.length > 0) {
        pushRow();
    }

    return hasHeader ? rows.slice(1) : rows;
};

export const parseBankAmount = (value: string) => {
    const cleaned = value
        .replace(/\u00a0/g, '')
        .replace(/\s/g, '')
        .replace(/[€$£]/g, '')
        .trim();

    if (!cleaned) throw new Error('Montant bancaire manquant.');

    const lastComma = cleaned.lastIndexOf(',');
    const lastDot = cleaned.lastIndexOf('.');
    const normalized = lastComma > -1 && lastDot > -1
        ? lastComma > lastDot
            ? cleaned.replace(/\./g, '').replace(',', '.')
            : cleaned.replace(/,/g, '')
        : cleaned.replace(',', '.');

    const amount = Number(normalized);
    if (!/^[+-]?(?:\d+(?:\.\d+)?|\.\d+)$/.test(normalized) || !Number.isFinite(amount)) {
        throw new Error(`Montant bancaire invalide : ${value}`);
    }
    return amount;
};

const validatedDate = (year: string, month: string, day: string) => {
    const result = `${year}-${month.padStart(2, '0')}-${day.padStart(2, '0')}`;
    const date = new Date(`${result}T00:00:00Z`);
    if (!Number.isFinite(date.getTime()) || date.toISOString().slice(0, 10) !== result) {
        throw new Error(`Date bancaire invalide : ${result}`);
    }
    return result;
};

export const parseBankDate = (value: string) => {
    const raw = value.trim();
    if (!raw) throw new Error('Date bancaire manquante.');

    const isoMatch = raw.match(/^(\d{4})[-/](\d{1,2})[-/](\d{1,2})(?:$|T\d{2}:\d{2})/);
    if (isoMatch) {
        const [, year, month, day] = isoMatch;
        return validatedDate(year, month, day);
    }

    const frenchMatch = raw.match(/^(\d{1,2})[/-](\d{1,2})[/-](\d{2,4})$/);
    if (frenchMatch) {
        const [, day, month, inputYear] = frenchMatch;
        const year = inputYear.length === 2 ? `20${inputYear}` : inputYear;
        return validatedDate(year, month, day);
    }

    throw new Error(`Date bancaire invalide : ${value}`);
};

export const parseQifDate = (value: string) => {
    const normalized = value.replace("'", '/').trim();
    return parseBankDate(normalized);
};

export const parseQifTransactions = (content: string): ParsedStatementTransaction[] => {
    const transactions: ParsedStatementTransaction[] = [];
    let current: Partial<ParsedStatementTransaction> = {};

    const commitCurrent = () => {
        if (Object.keys(current).length === 0) return;

        if (!current.date || current.amount === undefined) throw new Error('Transaction QIF sans date ou montant.');
        transactions.push({
            date: current.date,
            amount: current.amount,
            description: current.description || 'Transaction QIF',
            category: current.category
        });
        current = {};
    };

    content.split(/\r?\n/).forEach(line => {
        const trimmed = line.trim();
        if (!trimmed || trimmed.startsWith('!')) return;

        const field = trimmed[0];
        const value = trimmed.slice(1).trim();

        if (field === '^') {
            commitCurrent();
        } else if (field === 'D') {
            current.date = parseQifDate(value);
        } else if (field === 'T') {
            current.amount = parseBankAmount(value);
        } else if (field === 'P' || field === 'M') {
            current.description = normalizeText(value);
        } else if (field === 'L') {
            current.category = normalizeText(value.replace(/^\[|\]$/g, ''));
        }
    });

    commitCurrent();

    if (transactions.length === 0) {
        throw new Error('Aucune transaction valide trouvée dans le fichier QIF.');
    }

    return transactions;
};

const getOfxTagValue = (block: string, tag: string) => {
    const match = block.match(new RegExp(`<${tag}>([^<\\r\\n]*)`, 'i'));
    return match ? decodeEntities(match[1].trim()) : '';
};

const parseOfxDate = (value: string) => {
    if (!/^\d{8}/.test(value)) throw new Error('Date OFX manquante ou invalide.');
    return validatedDate(value.slice(0, 4), value.slice(4, 6), value.slice(6, 8));
};

export const parseOfxTransactions = (content: string): ParsedStatementTransaction[] => {
    const blocks = [...content.matchAll(/<STMTTRN>([\s\S]*?)(?=<STMTTRN>|$)/gi)];
    const fid = getOfxTagValue(content, 'FID');
    const org = getOfxTagValue(content, 'ORG');

    if (blocks.length === 0) {
        throw new Error('Aucune transaction trouvée dans le fichier OFX. Le format est peut-être incorrect.');
    }

    return blocks.reduce<ParsedStatementTransaction[]>((transactions, match) => {
        const block = match[1];
        const prefix = content.slice(0, match.index);
        const statements = [...prefix.matchAll(/<(?:STMTRS|CCSTMTRS)>/gi)];
        const statement = prefix.slice(statements[statements.length - 1]?.index ?? 0);
        const acctId = getOfxTagValue(statement, 'ACCTID');
        const fitId = getOfxTagValue(block, 'FITID');
        const date = parseOfxDate(getOfxTagValue(block, 'DTPOSTED'));
        const amount = parseBankAmount(getOfxTagValue(block, 'TRNAMT'));
        const name = normalizeText(getOfxTagValue(block, 'NAME'));
        const memo = normalizeText(getOfxTagValue(block, 'MEMO'));

        if (!date || !Number.isFinite(amount)) return transactions;

        const description = memo && memo !== name
            ? normalizeText([name, memo].filter(Boolean).join(' - '))
            : name || memo || 'Transaction OFX';

        const identity = acctId && fitId ? {
            bankSource: 'ofx:' + JSON.stringify([fid, org, getOfxTagValue(statement, 'BANKID'), getOfxTagValue(statement, 'BRANCHID'), acctId, getOfxTagValue(statement, 'ACCTTYPE')]),
            bankTransactionId: fitId,
        } : {};
        transactions.push({ date, amount, description, ...identity });
        return transactions;
    }, []);
};

const normalizeFingerprintText = (value: string) => normalizeText(value)
    .toLowerCase()
    .normalize('NFD')
    .replace(/[\u0300-\u036f]/g, '');

export const transactionFingerprint = (
    transaction: Pick<ImportTransactionInput | Transaction, 'date' | 'amount' | 'type' | 'description'> & { accountId?: string },
    fallbackAccountId?: string
) => [
    transaction.date,
    transaction.accountId || fallbackAccountId || '',
    transaction.type,
    Math.round((transaction.amount || 0) * 100),
    normalizeFingerprintText(transaction.description || '')
].join('|');

export const filterDuplicateTransactions = <T extends ImportTransactionInput>(
    incoming: T[],
    existing: Transaction[],
    fallbackAccountId: string
) => {
    // Match occurrences, not just values: two identical purchases on a statement
    // are legitimate. Re-importing the same statement must still add neither twice.
    const identity = (transaction: T | Transaction) => transaction.bankSource && transaction.bankTransactionId
        ? JSON.stringify([transaction.accountId || fallbackAccountId, transaction.bankSource, transaction.bankTransactionId]) : null;
    const existingIds = new Set(existing.map(transaction => transaction.id));
    const seenIdentities = new Set(existing.map(identity).filter((value): value is string => value !== null));
    const remaining = new Map<string, number>();
    existing.forEach(transaction => {
        const key = transactionFingerprint(transaction);
        remaining.set(key, (remaining.get(key) || 0) + 1);
    });
    const unique: T[] = [];
    let duplicateCount = 0;

    incoming.forEach(transaction => {
        const bankIdentity = identity(transaction);
        if (bankIdentity) {
            if (seenIdentities.has(bankIdentity) || existingIds.has(uuidv5(bankIdentity, uuidv5.URL))) duplicateCount++;
            else { unique.push(transaction); seenIdentities.add(bankIdentity); }
            return;
        }
        const key = transactionFingerprint(transaction, fallbackAccountId);
        const count = remaining.get(key) || 0;
        if (count > 0) {
            remaining.set(key, count - 1);
            duplicateCount++;
            return;
        }

        unique.push(transaction);
    });

    return { unique, duplicateCount };
};
