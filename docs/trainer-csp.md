# Trainer content security policy

The packaged WebView now uses an explicit CSP instead of `null`:

- scripts, fonts and ordinary resources are local (`'self'`);
- IPC may connect to `ipc:` and `http://ipc.localhost` (Tauri transport);
- no remote HTTPS origins, eval, plugins, child frames or form submissions;
- images may additionally be data URLs;
- inline **styles only** are allowed because Motion and React set dynamic layout
  properties; scripts do not receive this exception;
- local development HMR WebSocket origins exist only in `devCsp`;
- Tauri asset CSP rewriting remains enabled; capabilities stay `core:default`.

`trainer/src/lib/content-security-policy.test.ts` is a configuration regression
suite, not a native WebView test. Frontend typecheck, Vitest and the production
Vite build verify the frontend/build tier. Native WebView loading, all IPC commands,
local data loading and lifecycle on each supported platform remain mandatory RC
acceptance; a passing configuration test alone does not close that gate.
