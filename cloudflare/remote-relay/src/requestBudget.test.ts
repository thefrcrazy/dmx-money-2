import { expect, test } from 'vitest';
import { BodyReadTimeout, limitedText, RequestBudget, reservedResponse } from './requestBudget';

test('reserves capacity before slow reads, including bytes held by pending replies', () => {
  const budget = new RequestBudget(2, 24);
  const first = budget.reserve(12)!;
  const second = budget.reserve(12)!;
  expect(budget.reserve(12)).toBeNull();
  // Small inputs retain full-frame reservations: their replies may still be large.
  expect(budget.reserve(1)).toBeNull();
  second.release();
  second.release();
  const reused = budget.reserve(12)!;
  expect(reused).not.toBeNull();
  expect(budget.reserve(1)).toBeNull();
  [first, reused].forEach(slot => slot.release());
  expect(budget.reserve(24)).not.toBeNull();
});

test('reply reservations survive fetch return and partial reads, then release at EOF or cancel', async () => {
  const budget = new RequestBudget(2, 24);
  const first = reservedResponse(new Response('first'), budget.reserve(12)!);
  const second = reservedResponse(new Response('second'), budget.reserve(12)!);
  expect(budget.reserve(12)).toBeNull();
  const reader = first.body!.getReader();
  expect((await reader.read()).done).toBe(false);
  expect(budget.reserve(12)).toBeNull();
  expect((await reader.read()).done).toBe(true);
  reader.releaseLock();
  const next = budget.reserve(12)!;
  expect(next).not.toBeNull();
  expect(budget.reserve(12)).toBeNull();
  await second.body!.cancel();
  next.release();
  expect(budget.reserve(24)).not.toBeNull();
});

test('reply body errors and empty responses release their reservations', async () => {
  const budget = new RequestBudget(1, 12);
  const errored = new ReadableStream<Uint8Array>({ pull(controller) { controller.error(new Error('fixture')); } });
  const output = reservedResponse(new Response(errored), budget.reserve(12)!);
  await expect(output.text()).rejects.toThrow('fixture');
  const slot = budget.reserve(12)!;
  expect(slot).not.toBeNull();
  reservedResponse(new Response(null, { status: 204 }), slot);
  expect(budget.reserve(12)).not.toBeNull();
});

test('slow bodies time out and are canceled without allocating a full frame', async () => {
  let canceled = false;
  const body = new ReadableStream<Uint8Array>({ cancel() { canceled = true; } });
  const request = new Request('https://fixture.invalid/', { method: 'POST', body });
  await expect(limitedText(request, 1024, 20)).rejects.toBeInstanceOf(BodyReadTimeout);
  expect(canceled).toBe(true);
});

test('body size is enforced for streamed chunks and malformed UTF-8 is rejected', async () => {
  const stream = (bytes: Uint8Array) => new ReadableStream<Uint8Array>({ start(controller) { controller.enqueue(bytes); controller.close(); } });
  expect(await limitedText(new Request('https://fixture.invalid/', { method: 'POST', body: stream(new Uint8Array(5)) }), 4)).toBeNull();
  await expect(limitedText(new Request('https://fixture.invalid/', { method: 'POST', body: stream(Uint8Array.of(255)) }), 4)).rejects.toThrow();
});
