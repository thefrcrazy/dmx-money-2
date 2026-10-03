export type RequestReservation = { release: () => void };

/** Keep a worst-case frame reserved through body reads, pending replies and response delivery. */
export class RequestBudget {
  private active = 0;
  private bytes = 0;

  constructor(private readonly maxActive: number, private readonly maxBytes: number) {}

  reserve(bytes: number): RequestReservation | null {
    if (this.active >= this.maxActive || this.bytes + bytes > this.maxBytes) return null;
    this.active += 1;
    this.bytes += bytes;
    let released = false;
    return {
      release: () => {
        if (released) return;
        released = true;
        this.active -= 1;
        this.bytes -= bytes;
      },
    };
  }
}

/** Transfer ownership to the output stream, including slow clients and canceled responses. */
export function reservedResponse(response: Response, reservation: RequestReservation): Response {
  if (!response.body) {
    reservation.release();
    return response;
  }
  const reader = response.body.getReader();
  let finished = false;
  const finish = () => {
    if (finished) return;
    finished = true;
    reservation.release();
    reader.releaseLock();
  };
  const body = new ReadableStream<Uint8Array>({
    async pull(controller) {
      try {
        const { done, value } = await reader.read();
        if (done) { finish(); controller.close(); }
        else controller.enqueue(value);
      } catch (error) {
        finish();
        controller.error(error);
      }
    },
    async cancel(reason) {
      try { await reader.cancel(reason); }
      finally { finish(); }
    },
  }, { highWaterMark: 0 });
  return new Response(body, { status: response.status, statusText: response.statusText, headers: response.headers });
}

export class BodyReadTimeout extends Error {}

export async function limitedText(request: Request, limit: number, timeoutMs = 10_000): Promise<string | null> {
  if (!request.body) return '';
  const reader = request.body.getReader();
  let timer: ReturnType<typeof setTimeout> | undefined;
  const timeout = new Promise<never>((_, reject) => {
    timer = setTimeout(() => {
      reject(new BodyReadTimeout());
      void reader.cancel().catch(() => undefined);
    }, timeoutMs);
  });
  try {
    const chunks: Uint8Array[] = [];
    let size = 0;
    while (true) {
      const { done, value } = await Promise.race([reader.read(), timeout]);
      if (done) break;
      size += value.byteLength;
      if (size > limit) {
        void reader.cancel().catch(() => undefined);
        return null;
      }
      chunks.push(value);
    }
    const bytes = new Uint8Array(size);
    let offset = 0;
    for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.length; }
    return new TextDecoder('utf-8', { fatal: true }).decode(bytes);
  } finally {
    clearTimeout(timer);
    reader.releaseLock();
  }
}
