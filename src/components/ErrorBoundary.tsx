import { Component, type ErrorInfo, type ReactNode } from "react";

interface Props {
  children: ReactNode;
}

interface State {
  error: Error | null;
}

/**
 * Top-level error boundary. Before v0.16.2 a render-time exception
 * anywhere in the tree (e.g. a Rules-of-Hooks violation in NoteEditor)
 * would unmount the entire app and leave the user staring at a blank
 * window with no recovery. The boundary catches the throw, shows a
 * recoverable message + Reload button, and logs the error so it surfaces
 * in `tauri-plugin-log` output for diagnostics.
 */
export class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    // eslint-disable-next-line no-console
    console.error("[Keepr] uncaught render error:", error, info.componentStack);
  }

  render() {
    if (!this.state.error) return this.props.children;
    return (
      <div className="h-full w-full grid place-items-center bg-white dark:bg-[#202124] text-gray-800 dark:text-gray-100 p-6">
        <div className="max-w-md w-full rounded-lg border border-gray-300 dark:border-[#5f6368] p-6 shadow-keep">
          <h1 className="text-lg font-medium mb-2">Something went wrong</h1>
          <p className="text-sm opacity-80 mb-4">
            Keepr hit an unexpected error and stopped rendering. Your notes
            are safe on disk — reload the window to recover.
          </p>
          <pre className="text-xs opacity-70 whitespace-pre-wrap break-words mb-4 max-h-40 overflow-y-auto">
            {this.state.error.message}
          </pre>
          <button
            type="button"
            onClick={() => window.location.reload()}
            className="px-3 py-1.5 rounded border border-gray-300 dark:border-[#5f6368] hover:bg-black/5 dark:hover:bg-white/10 text-sm"
          >
            Reload
          </button>
        </div>
      </div>
    );
  }
}
