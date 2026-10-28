import { beforeEach, describe, expect, it, vi } from 'vitest';

const { makeLocalApiRequest } = vi.hoisted(() => ({
  makeLocalApiRequest: vi.fn(),
}));
vi.mock('@/shared/lib/localApiTransport', () => ({ makeLocalApiRequest }));

import { toolsApi } from './toolsApi';

const ok = (data: unknown) =>
  new Response(JSON.stringify({ success: true, data }), { status: 200 });

describe('toolsApi', () => {
  beforeEach(() => makeLocalApiRequest.mockReset());

  it('always targets the local backend, never the host being viewed', async () => {
    makeLocalApiRequest
      .mockResolvedValueOnce(ok({ tools: [], errors: [] }))
      .mockResolvedValueOnce(ok({ items: [] }))
      .mockResolvedValueOnce(
        ok({ status: 'exited', exit_code: 0, output: '', log_path: '' })
      );

    await toolsApi.list();
    await toolsApi.menu({ tool_id: 't', target: 'click' });
    await toolsApi.run({ tool_id: 't', target: 'click' });

    expect(makeLocalApiRequest).toHaveBeenCalledTimes(3);
    for (const [, init] of makeLocalApiRequest.mock.calls) {
      expect(init).toMatchObject({ hostScope: 'none' });
    }
  });

  it('surfaces the server message on failure', async () => {
    makeLocalApiRequest.mockResolvedValueOnce(
      new Response(JSON.stringify({ success: false, message: 'boom' }), {
        status: 400,
      })
    );
    await expect(toolsApi.list()).rejects.toThrow('boom');
  });
});
