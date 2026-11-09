import { app, safeStorage } from "electron";
import {
  existsSync,
  readFileSync,
  writeFileSync,
  renameSync,
  unlinkSync,
} from "node:fs";
import path from "node:path";
import type { EngineManager } from "./engine-manager";
import type { CloudProfileStatus, CloudStatus, UsageCloudState } from "../shared/desktop-api";
import { CLOUD_URL as BASE_URL } from "./cloud-url";

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
        heldBack: [], heldBackCount: 0, conflicts: [],
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
    if (
      !safeStorage.isEncryptionAvailable() ||
      (process.platform === "linux" &&
        safeStorage.getSelectedStorageBackend() === "basic_text")
    )
      throw new Error(
        "Unlock your system keyring to save your sign-in securely.",
      );
    this.error = null;
    const response = await this.request("/api/auth/sign-in", {
      email,
      password,
    });
    if (typeof response.access_token !== "string" || !response.access_token)
      throw new Error("Sign-in failed. Try again.");
    const user = response.user as { id?: unknown; email?: unknown } | undefined;
    if (typeof user?.id !== "string" || typeof user.email !== "string")
      throw new Error("Could not verify your account.");
    const session: Session = {
      token: response.access_token,
      user: { id: user.id, email: user.email },
    };
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
