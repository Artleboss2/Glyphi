import { app, safeStorage, shell } from "electron";
import {
  existsSync,
  readFileSync,
  writeFileSync,
  renameSync,
  unlinkSync,
} from "node:fs";
import { createServer } from "node:http";
import path from "node:path";
import type { EngineManager } from "./engine-manager";
import type { CloudProfileStatus, CloudStatus, UsageCloudState } from "../shared/desktop-api";
import { CLOUD_URL as BASE_URL } from "./cloud-url";

/** How long the loopback listener waits for the browser to come back. */
const GITHUB_SIGN_IN_TIMEOUT_MS = 5 * 60 * 1000;

export type KnownProfileStatus = Extract<CloudProfileStatus, { state: "missing" | "private" | "public" }>;

const PROFILE_HANDLE = /^[a-z0-9][a-z0-9_]{2,29}$/u;
interface Session {
  token: string;
  user: { id: string; email: string };
}

export class CloudAccount {
  private session: Session | null = null;
  private error: string | null = null;
  private profile: { accountId: string; status: KnownProfileStatus } | null = null;
  private profileCheck: { session: Session; done: Promise<void> } | null = null;
  private readonly filename = path.join(
    app.getPath("userData"),
    "cloud-session.enc",
  );
  constructor(
    private readonly engine: EngineManager,
    private readonly configureEngine: (
      accountId: string, operation: () => Promise<unknown>, reconnect: boolean
    ) => Promise<unknown> = (_accountId, operation) => operation(),
    private readonly onProfile: (accountId: string, profile: KnownProfileStatus) => void = () => undefined,
  ) {}
  async initialize(): Promise<void> {
    try {
      if (existsSync(this.filename) && safeStorage.isEncryptionAvailable()) {
        const value = JSON.parse(
          safeStorage.decryptString(readFileSync(this.filename)),
        ) as Session;
        if (
          typeof value.token === "string" &&
          typeof value.user?.id === "string" &&
          typeof value.user.email === "string"
        )
          this.session = value;
      }
    } catch {
      this.error = "Your saved sign-in could not be opened. Sign in again.";
    }
    await this.configure();
  }
  async status(): Promise<CloudStatus> {
    const state = (await this.engine.cloud({
      operation: "status",
    })) as unknown as UsageCloudState;
    const account = this.session?.user ?? null;
    if (state.accountId !== (account?.id ?? null)) {
      return {
        accountId: null, account: null, automatic: false, pending: 0,
        busy: this.error === null, progress: null, lastCompleted: null,
        conflicts: [],
        error: this.error, profile: { state: "unknown" },
      };
    }
    return {
      ...state,
      account,
      error: this.error ?? state.error,
      profile: this.profileStatus(),
    };
  }
  profileStatus(): CloudProfileStatus {
    if (!this.session) return { state: "signed-out" };
    return this.profile?.accountId === this.session.user.id ? this.profile.status : { state: "unknown" };
  }
  /** Asks the website whether the signed-in account has a profile. Failures keep the last answer. */
  refreshProfile(): Promise<void> {
    const session = this.session;
    if (!session) return Promise.resolve();
    if (this.profileCheck?.session !== session) {
      const done = (async () => {
        try {
          const value = await this.request("/api/desktop/profile", undefined, session.token);
          const profile = parseProfile(value.profile);
          if (this.session !== session) return;
          this.profile = { accountId: session.user.id, status: profile };
          this.onProfile(session.user.id, profile);
        } catch { /* An unknown profile never counts as missing. */ }
      })().finally(() => { if (this.profileCheck?.done === done) this.profileCheck = null; });
      this.profileCheck = { session, done };
    }
    return this.profileCheck.done;
  }
  /**
   * Whether the handle looks free. Needs no account, so a signed-out user can choose first,
   * but then only public profiles count; signed in, private ones count too, as saving would.
   */
  async handleAvailable(handle: string): Promise<boolean> {
    if (!PROFILE_HANDLE.test(handle)) throw new Error("Invalid handle.");
    const value = await this.request(`/api/handles/${handle}`, undefined, this.session?.token);
    if (typeof value.available !== "boolean") throw new Error("Invalid handle response.");
    return value.available;
  }

  /**
   * Signs in from the app itself. A first sign-in creates the free account; the
   * token lands in the OS keyring, so it never touches a plain file.
   */
  async signIn(email: string, password: string): Promise<void> {
    if (this.session) return;
    this.requireKeyring();
    this.error = null;
    await this.adopt(await this.request("/api/auth/sign-in", { email, password }));
  }

  /**
   * Signs in through GitHub in the browser, the way native apps do: the app
   * listens on a loopback port, the API sends the browser back to it with a
   * one-time ticket, and the ticket is exchanged here for a session. The
   * password never touches the app and no token ever crosses the browser.
   */
  async signInWithGitHub(): Promise<void> {
    if (this.session) return;
    this.requireKeyring();
    this.error = null;

    const listener = await this.listenForTicket();
    const target = new URL(`${BASE_URL}/api/auth/github/start`);
    target.searchParams.set("port", String(listener.port));
    try {
      await shell.openExternal(target.toString());
    } catch {
      listener.close();
      throw new Error(
        "Glyphi could not open your browser. Open it, then try again.",
      );
    }

    try {
      const ticket = await listener.ticket;
      const answer = await this.request("/api/auth/github/exchange", {
        ticket,
      });
      const token = answer.access_token;
      if (typeof token !== "string" || !token)
        throw new Error("Sign-in failed. Try again.");
      // The exchange returns the token alone; the account comes from the session
      // it unlocks, which also proves the token is usable before it is stored.
      const session = await this.request(
        "/api/auth/get-session",
        undefined,
        token,
      );
      await this.adopt({ access_token: token, user: session.user as {
        id?: unknown;
        email?: unknown;
      } | undefined });
    } finally {
      listener.close();
    }
  }

  /**
   * One loopback listener for the redirect back from GitHub. It only answers
   * this machine, and only for a well-formed ticket; anything else is ignored.
   */
  private async listenForTicket(): Promise<{
    port: number;
    ticket: Promise<string>;
    close: () => void;
  }> {
    let settle!: { resolve: (ticket: string) => void; reject: (error: Error) => void };
    const ticket = new Promise<string>((resolve, reject) => {
      settle = { resolve, reject };
    });
    // The rejection is reported through the await below; this only keeps Node
    // from warning about it while the browser is still opening.
    ticket.catch(() => undefined);

    const server = createServer((request, response) => {
      const url = new URL(request.url ?? "/", "http://127.0.0.1");
      const remote = request.socket.remoteAddress ?? "";
      if (
        url.pathname !== "/callback" ||
        !/^(127\.0\.0\.1|::1|::ffff:127\.0\.0\.1)$/u.test(remote)
      ) {
        response.writeHead(404, { connection: "close" }).end();
        return;
      }
      const value = url.searchParams.get("ticket") ?? "";
      if (!/^[a-f0-9]{32,64}$/u.test(value)) {
        response
          .writeHead(400, {
            "content-type": "text/plain; charset=utf-8",
            connection: "close",
          })
          .end("That sign-in link is invalid. Start again from Glyphi.");
        return;
      }
      response
        .writeHead(200, {
          "content-type": "text/html; charset=utf-8",
          connection: "close",
        })
        .end(
          "<!doctype html><meta charset=utf-8><title>Glyphi</title>" +
            '<body style="margin:0;min-height:100vh;display:grid;place-items:center;background:#12100d;color:#f2e7d5;font:16px/1.6 system-ui,sans-serif">' +
            '<main style="text-align:center"><h1 style="font-weight:600;font-size:20px">You are signed in.</h1>' +
            '<p style="color:#b9a88f">Return to Glyphi — this tab can be closed.</p></main>',
        );
      settle.resolve(value);
    });

    const port = await new Promise<number>((resolve, reject) => {
      server.once("error", () =>
        reject(
          new Error("Glyphi could not listen for the sign-in. Try again."),
        ),
      );
      server.listen(0, "127.0.0.1", () => {
        const address = server.address();
        if (address && typeof address === "object") resolve(address.port);
        else
          reject(
            new Error("Glyphi could not listen for the sign-in. Try again."),
          );
      });
    });

    const timer = setTimeout(
      () => settle.reject(new Error("Sign-in timed out. Try again.")),
      GITHUB_SIGN_IN_TIMEOUT_MS,
    );
    timer.unref();
    return {
      port,
      ticket,
      close: () => {
        clearTimeout(timer);
        server.close();
      },
    };
  }

  /** A session is only worth having if the OS can keep it out of plain files. */
  private requireKeyring(): void {
    if (
      !safeStorage.isEncryptionAvailable() ||
      (process.platform === "linux" &&
        safeStorage.getSelectedStorageBackend() === "basic_text")
    )
      throw new Error(
        "Unlock your system keyring to save your sign-in securely.",
      );
  }

  /** Checks the answer, then parks the session in the OS keyring. */
  private async adopt(answer: {
    access_token?: unknown;
    user?: { id?: unknown; email?: unknown } | undefined;
  }): Promise<void> {
    const token = answer.access_token;
    const user = answer.user;
    if (typeof token !== "string" || !token)
      throw new Error("Sign-in failed. Try again.");
    if (typeof user?.id !== "string" || typeof user.email !== "string")
      throw new Error("Could not verify your account.");
    const session: Session = { token, user: { id: user.id, email: user.email } };
    writeFileSync(
      `${this.filename}.tmp`,
      safeStorage.encryptString(JSON.stringify(session)),
      { mode: 0o600 },
    );
    renameSync(`${this.filename}.tmp`, this.filename);
    this.session = session;
    await this.configure();
    // The profile page reads this straight after sign-in; a failed check stays quiet.
    await this.refreshProfile();
  }

  async signOut(): Promise<void> {
    const session = this.session;
    this.session = null;
    this.error = null;
    let removalError: Error | null = null;
    try {
      if (existsSync(this.filename)) unlinkSync(this.filename);
    } catch {
      removalError = new Error("The saved sign-in could not be removed. Close other Glyphi windows and try signing out again.");
    }
    // A locked session file must not leave automatic cloud saves connected.
    try {
      await this.configure();
    } catch (error) {
      this.error = error instanceof Error ? error.message : "Could not disconnect cloud sync. Close Glyphi and try again.";
      throw error;
    } finally {
      if (session)
        await this.request("/api/auth/sign-out", {}, session.token).catch(() => undefined);
    }
    if (removalError) {
      this.error = removalError.message;
      throw removalError;
    }
  }
  async operation(
    operation: "save" | "restore" | "automatic" | "resolve",
    values: {
      enabled?: boolean;
      recordId?: string;
      choice?: "local" | "cloud";
    } = {},
  ): Promise<void> {
    if (!this.session) throw new Error("Sign in first.");
    this.error = null;
    await this.engine.cloud({ operation, ...values });
  }
  async reconnect(): Promise<void> {
    await this.configure(true);
  }
  private configure(reconnect = false): Promise<unknown> {
    const params = {
      operation: "configure",
      accountId: this.session?.user.id ?? "",
      token: this.session?.token ?? "",
      baseURL: BASE_URL,
    };
    return this.configureEngine(params.accountId, () => this.engine.cloud(params), reconnect);
  }
  private async request(
    endpoint: string,
    body?: unknown,
    token?: string,
  ): Promise<Record<string, unknown>> {
    let response: Response;
    try {
      response = await fetch(`${BASE_URL}${endpoint}`, {
        method: body ? "POST" : "GET",
        headers: {
          "Content-Type": "application/json",
          ...(token ? { Authorization: `Bearer ${token}` } : {}),
        },
        body: body ? JSON.stringify(body) : undefined,
        signal: AbortSignal.timeout(20_000),
        redirect: "error",
      });
    } catch {
      throw new Error(
        "Could not reach Glyphi. Check your connection and try again.",
      );
    }
    const reader = response.body?.getReader();
    if (!reader) throw new Error("Empty account response.");
    const decoder = new TextDecoder();
    let text = "";
    let size = 0;
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      size += value.byteLength;
      if (size > 65_536) {
        await reader.cancel();
        throw new Error("Invalid account response.");
      }
      text += decoder.decode(value, { stream: true });
    }
    text += decoder.decode();
    const value = JSON.parse(text) as Record<string, unknown>;
    if (!value || typeof value !== "object")
      throw new Error("Invalid account response.");
    if (!response.ok) {
      // The API explains a rejected password; anything else is opaque on purpose.
      if (value.error === "sign_in_failed" && typeof value.message === "string" && value.message)
        throw new Error(value.message);
      throw new Error(
        response.status === 503
          ? "Cloud accounts are not configured yet."
          : "Account request failed. Try again.",
      );
    }
    return value;
  }
}

function parseProfile(value: unknown): KnownProfileStatus {
  if (value === null) return { state: "missing" };
  if (value && typeof value === "object" && "handle" in value && "isPublic" in value
    && typeof value.handle === "string" && PROFILE_HANDLE.test(value.handle)
    && typeof value.isPublic === "boolean")
    return { state: value.isPublic ? "public" : "private", handle: value.handle };
  throw new Error("Invalid profile response.");
}
