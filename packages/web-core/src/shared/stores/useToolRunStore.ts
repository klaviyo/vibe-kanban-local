import { create } from 'zustand';

interface ToolRunState {
  /** Number of in-flight runs per tool id. */
  running: Record<string, number>;
  start: (toolId: string) => void;
  finish: (toolId: string) => void;
}

export const useToolRunStore = create<ToolRunState>()((set) => ({
  running: {},
  start: (toolId) =>
    set((s) => ({
      running: { ...s.running, [toolId]: (s.running[toolId] ?? 0) + 1 },
    })),
  finish: (toolId) =>
    set((s) => ({
      running: {
        ...s.running,
        [toolId]: Math.max(0, (s.running[toolId] ?? 0) - 1),
      },
    })),
}));
