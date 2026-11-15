/*
 * window.glyphi, without Electron.
 *
 * The React renderer was written against a preload that exposed this object, so the
 * shell injects the same names. Nothing here knows about WebView2 or WKWebView: it
 * posts `{ id, method, params }` through the IPC channel and resolves when the
 * engine answers on the same id.
 *
 * Methods the Rust core has not ported yet answer a safe default instead of
 * throwing. A missing feature must degrade one card, not blank the dashboard, so
 * every default below has the same shape the renderer parses — an empty object
 * where a profile belongs is enough to throw inside `JSON.stringify`.
 */
(function () {
  if (window.glyphi) return;

  var pending = new Map();
  var counter = 0;
  var snapshotListeners = new Set();

  function post(method, params) {
    return new Promise(function (resolve, reject) {
      var id = String(++counter);
      pending.set(id, { resolve: resolve, reject: reject });
      window.ipc.postMessage(JSON.stringify({ id: id, method: method, params: params || {} }));
    });
  }

  window.__glyphiBridge = {
    receive: function (payload) {
      if (!payload || !payload.id) return;
      var entry = pending.get(payload.id);
      if (!entry) return;
      pending.delete(payload.id);
      if (payload.ok) {
        entry.resolve(payload.result);
        return;
      }
      var error = new Error((payload.error && payload.error.message) || "The engine failed.");
      error.code = payload.error && payload.error.code;
      error.retryable = Boolean(payload.error && payload.error.retryable);
      entry.reject(error);
    }
  };

  /* The shell pushes a dashboard it refreshed on its own — at start-up, from the
     tray — through the same subscription the renderer already registered. */
  window.__glyphiPush = function (state) {
    snapshotListeners.forEach(function (listener) {
      try {
        listener(state);
      } catch (error) {
        /* One broken listener must not stop the others. */
      }
    });
  };

  /* Settles with the default when the engine does not answer within `ms`, so a
     stuck engine cannot leave an awaited boot sequence hanging forever. */
  function withTimeout(promise, ms, fallback) {
    return Promise.race([
      promise,
      new Promise(function (resolve) {
        setTimeout(function () {
          resolve(fallback);
        }, ms);
      })
    ]);
  }

  var api = {
    platform: "rust",
    /* The engine. */
    getSnapshot: function () {
      return post("glyphi.snapshot");
    },
    refreshAll: function () {
      return post("glyphi.refreshAll");
    },
    refreshProvider: function (providerId) {
      return post("glyphi.refreshProvider", { providerId: providerId });
    },
    updatePreferences: function (patch) {
      return post("shell.preferences", { patch: patch || {} });
    },
    /* Read from the same JSON file the Electron build wrote, so a theme, a
       language and a background survive the port. */
    getPreferences: function () {
      return withTimeout(post("shell.preferences"), 1500, null);
    },
    /* Window controls: the renderer draws the title bar. */
    minimizeWindow: function () {
      return post("window.minimize");
    },
    toggleMaximizeWindow: function () {
      return post("window.toggleMaximize");
    },
    closeWindow: function () {
      return post("window.close");
    },
    beginDrag: function () {
      return post("window.drag");
    },
    toggleWidget: function () {
      return post("window.toggle");
    },
    /* Not in the Rust core yet. */
    getAppMarks: function () {
      return Promise.resolve({});
    },
    getDiagnostics: function () {
      return Promise.resolve({ status: "ready", restartCount: 0, messages: [] });
    },
    getWindowState: function () {
      return Promise.resolve({ maximized: false });
    },
    getUpdateState: function () {
      return Promise.resolve({ status: "unavailable", currentVersion: null });
    },
    checkForUpdates: function () {
      return Promise.resolve({ status: "unavailable" });
    },
    installUpdate: function () {
      return Promise.resolve(false);
    },
    rollbackUpdate: function () {
      return Promise.resolve(false);
    },
    /* The full signed-out shape. `profile` is a union with no null branch: the
       renderer reads `status.profile.state`, so a null there blanks the window. */
    getCloudStatus: function () {
      return Promise.resolve({
        signedIn: false,
        accountId: null,
        automatic: false,
        pending: 0,
        busy: false,
        progress: null,
        lastCompleted: null,
        error: null,
        conflicts: [],
        account: null,
        profile: { state: "signed-out" }
      });
    },
    cloudAction: function () {
      return Promise.resolve(null);
    },
    getLeaderboard: function () {
      return Promise.resolve(null);
    },
    listDiscoverableProviders: function () {
      return Promise.resolve([]);
    },
    scanInstalledProviders: function () {
      return Promise.resolve([]);
    },
    getProviderDirectory: function () {
      return Promise.resolve([]);
    },
    discoverProvider: function () {
      return Promise.resolve(null);
    },
    setProviderEnabled: function () {
      return Promise.resolve(true);
    },
    setProviderCredential: function () {
      return Promise.resolve(true);
    },
    getProviderCredentials: function () {
      return Promise.resolve([]);
    },
    setReportPlans: function () {
      return Promise.resolve(true);
    },
    getCustomBackground: function () {
      return Promise.resolve(null);
    },
    chooseCustomBackground: function () {
      return Promise.resolve(null);
    },
    chooseFolder: function () {
      return Promise.resolve(null);
    },
    chooseExportDirectory: function () {
      return Promise.resolve(null);
    },
    exportSettings: function () {
      return Promise.resolve(false);
    },
    importSettings: function () {
      return Promise.resolve(false);
    },
    exportUsageNow: function () {
      return Promise.resolve(false);
    },
    copyText: function (text) {
      return navigator.clipboard.writeText(String(text)).then(function () {
        return true;
      });
    },
    copyImage: function () {
      return Promise.resolve(false);
    },
    saveImage: function () {
      return Promise.resolve(false);
    },
    openExternal: function (url) {
      window.open(url, "_blank", "noopener");
      return Promise.resolve(true);
    },
    restartApp: function () {
      return Promise.resolve(false);
    },
    trackGrowthEvent: function () {
      return Promise.resolve(true);
    },
    sendTestAlert: function () {
      return Promise.resolve(true);
    },
    showWidget: function () {
      return post("window.toggle");
    },
    hideWidget: function () {
      return post("window.toggle");
    },
    expandWidget: function () {
      return Promise.resolve(true);
    },
    beginWidgetResize: function () {
      return Promise.resolve(true);
    },
    endWidgetResize: function () {
      return Promise.resolve(true);
    },
    /* Subscriptions the shell can feed. The rest stay silent so the renderer
       keeps its unsubscribe contract. */
    onSnapshot: function (listener) {
      snapshotListeners.add(listener);
      return function () {
        snapshotListeners.delete(listener);
      };
    },
    onEngineStatus: function () {
      return function () {};
    },
    onNavigate: function () {
      return function () {};
    },
    onUsageAlert: function () {
      return function () {};
    },
    onUpdateState: function () {
      return function () {};
    },
    onWindowMaximized: function () {
      return function () {};
    }
  };

  window.glyphi = Object.freeze(api);
})();
