import { create } from 'zustand';

export type ToastTone = 'info' | 'error';

export interface Toast {
  id: number;
  title: string;
  detail?: string;
  tone: ToastTone;
}

interface ToastState {
  toasts: Toast[];
  show: (toast: Omit<Toast, 'id'>) => void;
  dismiss: (id: number) => void;
}

let nextToastId = 1;
const TOAST_LIFETIME_MS = 8000;

export const useToastStore = create<ToastState>()((set, get) => ({
  toasts: [],
  show: (toast) => {
    const id = nextToastId++;
    set({ toasts: [...get().toasts, { ...toast, id }] });
    setTimeout(() => get().dismiss(id), TOAST_LIFETIME_MS);
  },
  dismiss: (id) => set({ toasts: get().toasts.filter((t) => t.id !== id) }),
}));
