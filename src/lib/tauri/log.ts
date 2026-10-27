import { invoke } from '@tauri-apps/api/core'
import { postLog } from './api'

const originalConsole = {
  log: console.log.bind(console),
  error: console.error.bind(console),
  warn: console.warn.bind(console),
  info: console.info.bind(console),
  debug: console.debug.bind(console)
}

function formatError(error: unknown): string {
  if (error instanceof Error) {
    const stack = error.stack || ''
    const cleanStack = stack
      .split('\n')
      .filter(line => !line.includes('node_modules') && !line.includes('@vite'))
      .slice(0, 10)
      .join('\n')
    return `${error.name}: ${error.message}\n${cleanStack}`
  }
  return String(error)
}

function formatArgs(args: unknown[]): string {
  return args
    .map((arg) => {
      if (arg === undefined) return 'undefined';
      if (arg === null) return 'null';
      if (typeof arg === 'function') return `[Function: ${arg.name || 'anonymous'}]`;
      if (typeof arg === 'object') {
        try {
          const seen = new WeakSet();
          return JSON.stringify(arg, (key, value) => {
            if (typeof value === 'object' && value !== null) {
              if (seen.has(value)) return '[Circular]';
              seen.add(value);
            }
            if (typeof value === 'function') return `[Function: ${value.name || 'anonymous'}]`;
            if (typeof value === 'undefined') return '[undefined]';
            return value;
          }, 2);
        } catch {          return `[Object: ${arg.constructor?.name || 'Unknown'}]`;
        }
      }
      return String(arg);
    })
    .join(' ');
}

function interceptConsole() {
  console.log = (...args: unknown[]) => {
    originalConsole.log(...args);
    postLog('info', formatArgs(args).substring(0, 1000), 'console.log').catch(e => {
      originalConsole.error('Failed to send log to Tauri:', e);
    });
  };

  console.info = (...args: unknown[]) => {
    originalConsole.info(...args);
    postLog('info', formatArgs(args).substring(0, 1000), 'console.info').catch(e => {
      originalConsole.error('Failed to send log to Tauri:', e);
    });
  };

  console.debug = (...args: unknown[]) => {
    originalConsole.debug(...args);
    postLog('debug', formatArgs(args).substring(0, 1000), 'console.debug').catch(e => {
      originalConsole.error('Failed to send log to Tauri:', e);
    });
  };

  console.warn = (...args: unknown[]) => {
    originalConsole.warn(...args);
    postLog('warn', formatArgs(args).substring(0, 1000), 'console.warn').catch(e => {
      originalConsole.error('Failed to send log to Tauri:', e);
    });
  };

  console.error = (...args: unknown[]) => {
    originalConsole.error(...args);
    postLog('error', formatArgs(args).substring(0, 1000), 'console.error').catch(e => {
      originalConsole.error('Failed to send log to Tauri:', e);
    });
  };
}

function interceptWindowErrors() {
  window.addEventListener('error', (event) => {
    if (event.message?.includes('[vite]')) {
      return;
    }
    const error = event.error || event;
    const message = `${event.message || 'Unknown error'}${error ? `\n${formatError(error)}` : ''}`;
    postLog('error', message, `window.error at ${event.filename}:${event.lineno}:${event.colno}`).catch(e => {
      originalConsole.error('Failed to send log to Tauri:', e);
    });
    invoke('post_history', { input: { kind: 'error', title: event.message || 'Unknown error', detail: `${error ? formatError(error) : ''}\n${event.filename}:${event.lineno}:${event.colno}`.trim(), href: location.pathname + location.search } }).catch(() => {});
  });

  window.addEventListener('unhandledrejection', (event) => {
    const reasonStr = String(event.reason);
    if (
      reasonStr.includes('[callbackId, data]') ||
      reasonStr.includes('IPC custom protocol') ||
      reasonStr.includes('module.default') ||
      reasonStr.includes('@vite/client')
    ) {
      return;
    }
    const message = `Unhandled Promise rejection: ${formatError(event.reason)}`;
    postLog('error', message, 'unhandledrejection').catch(e => {
      originalConsole.error('Failed to send log to Tauri:', e);
    });
    invoke('post_history', { input: { kind: 'error', title: formatError(event.reason).split('\n')[0] || 'Unhandled rejection', detail: formatError(event.reason), href: location.pathname + location.search } }).catch(() => {});
  });
}

export function initLogger() {
  interceptConsole();
  interceptWindowErrors()
}