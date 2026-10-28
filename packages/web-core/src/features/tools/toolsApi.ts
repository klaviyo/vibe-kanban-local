import { makeLocalApiRequest } from '@/shared/lib/localApiTransport';

export interface ToolOption {
  id: string;
  title: string;
}

export interface ToolTarget {
  options: ToolOption[];
  /** The menu is produced by a script each time it opens. */
  dynamic: boolean;
}

export type MenuTone = 'ok' | 'warn' | 'error' | 'info';

export type MenuItem =
  | { type: 'option'; id: string; title: string; disabled: boolean }
  | { type: 'text'; text: string; tone: MenuTone | null }
  | { type: 'header'; text: string }
  | { type: 'separator' };

export interface MenuRequest {
  tool_id: string;
  target: 'click' | 'ticket';
  issue_id?: string;
}

export interface Tool {
  id: string;
  title: string;
  icon: string | null;
  color: string | null;
  click: ToolTarget;
  ticket: ToolTarget;
}

export interface ToolsResponse {
  tools: Tool[];
  errors: string[];
}

export interface RunToolRequest {
  tool_id: string;
  target: 'click' | 'ticket';
  option_id?: string;
  issue_id?: string;
}

export interface RunToolResult {
  status: 'exited' | 'running';
  exit_code: number | null;
  output: string;
  log_path: string;
}

async function unwrap<T>(response: Response): Promise<T> {
  const body = await response.json().catch(() => null);
  if (!response.ok || !body?.success) {
    throw new Error(body?.message ?? `Request failed (${response.status})`);
  }
  return body.data as T;
}

export const toolsApi = {
  list: async (): Promise<ToolsResponse> =>
    unwrap<ToolsResponse>(await makeLocalApiRequest('/api/tools')),
  menu: async (request: MenuRequest): Promise<MenuItem[]> =>
    (
      await unwrap<{ items: MenuItem[] }>(
        await makeLocalApiRequest('/api/tools/menu', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(request),
        })
      )
    ).items,
  run: async (request: RunToolRequest): Promise<RunToolResult> =>
    unwrap<RunToolResult>(
      await makeLocalApiRequest('/api/tools/run', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(request),
      })
    ),
};
