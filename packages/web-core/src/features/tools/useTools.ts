import { useCallback } from 'react';
import { useQuery } from '@tanstack/react-query';
import { useToastStore } from '@/shared/stores/useToastStore';
import { toolsApi, type RunToolRequest, type Tool } from './toolsApi';

const TOOLS_QUERY_KEY = ['tools'];

export function useTools() {
  const { data } = useQuery({
    queryKey: TOOLS_QUERY_KEY,
    queryFn: toolsApi.list,
    // The config file is read on every request; refetching on focus makes
    // edits show up without a restart.
    staleTime: 0,
    refetchOnWindowFocus: true,
  });
  const showToast = useToastStore((s) => s.show);

  const runTool = useCallback(
    async (tool: Tool, request: Omit<RunToolRequest, 'tool_id'>) => {
      try {
        const result = await toolsApi.run({ tool_id: tool.id, ...request });
        const detail = result.output || undefined;
        if (result.status === 'running') {
          showToast({
            title: `${tool.title} is still running`,
            detail: detail ?? result.log_path,
            tone: 'info',
          });
        } else if (result.exit_code === 0) {
          showToast({ title: `${tool.title}: done`, detail, tone: 'info' });
        } else {
          showToast({
            title: `${tool.title} failed (exit ${result.exit_code})`,
            detail,
            tone: 'error',
          });
        }
      } catch (err) {
        showToast({
          title: `${tool.title} failed`,
          detail: err instanceof Error ? err.message : String(err),
          tone: 'error',
        });
      }
    },
    [showToast]
  );

  return {
    tools: data?.tools ?? [],
    configErrors: data?.errors ?? [],
    runTool,
  };
}
