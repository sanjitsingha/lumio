// Small transient messages ("Saved", errors) for the Settings window.
let toasts = [];
let nextId = 1;
const listeners = new Set();

function emit() {
  listeners.forEach((l) => l());
}

export const toastStore = {
  subscribe(listener) {
    listeners.add(listener);
    return () => listeners.delete(listener);
  },
  getState() {
    return toasts;
  },
  push(message, kind = "info", timeout = kind === "error" ? 6000 : 2800) {
    const id = nextId++;
    toasts = [...toasts, { id, message, kind }];
    emit();
    setTimeout(() => toastStore.dismiss(id), timeout);
    return id;
  },
  dismiss(id) {
    toasts = toasts.filter((t) => t.id !== id);
    emit();
  },
};

export const toast = {
  info: (m) => toastStore.push(m, "info"),
  success: (m) => toastStore.push(m, "success"),
  error: (m) => toastStore.push(m, "error"),
};
