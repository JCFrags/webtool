// Node.js 24+ runs this TypeScript example directly. No packages are required.
// These are useful partial types, not a generated SDK or runtime schema validator.
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { basename } from "node:path";
import { parseArgs } from "node:util";
import { pathToFileURL } from "node:url";

type Problem = { code: string; message: string };
type Document = {
  id: string;
  title: string;
  source: { original: { sha256: string; size: number } };
  blocks: Array<{ id: string; content: { type: string }; locator: { kind: string } }>;
  warnings: Problem[];
};

export class ProblemError extends Error {
  status: number;
  code: string;
  constructor(status: number, problem: Problem) {
    super(`HTTP ${status} ${problem.code}: ${problem.message}`);
    this.status = status;
    this.code = problem.code;
  }
}

export class Webtool {
  server: string;
  constructor(server = "http://127.0.0.1:8420") {
    this.server = server.replace(/\/$/, "");
  }

  async request(path: string, init: RequestInit = {}): Promise<Response> {
    const response = await fetch(this.server + path, { ...init, signal: AbortSignal.timeout(150_000) });
    if (!response.ok) {
      let problem: Problem = { code: "http_error", message: "Non-Problem error response" };
      if (response.headers.get("content-type")?.includes("application/json")) {
        const value: unknown = await response.json().catch(() => null);
        if (value && typeof value === "object" && "code" in value && "message" in value
            && typeof value.code === "string" && typeof value.message === "string") {
          problem = { code: value.code, message: value.message };
        }
      }
      throw new ProblemError(response.status, problem);
    }
    return response;
  }

  async read(url: string): Promise<{ document: Document; cached: boolean }> {
    const response = await this.request("/v1/read", {
      method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ url }),
    });
    return response.json();
  }

  async ingest(file: string): Promise<Document> {
    const form = new FormData();
    form.set("name", basename(file));
    form.set("file", new Blob([await readFile(file)]), "upload.bin");
    // Fetch supplies the multipart boundary. Do not set Content-Type by hand.
    return (await this.request("/v1/ingest", { method: "POST", body: form })).json();
  }

  async document(id: string): Promise<Document> {
    return (await this.request(`/v1/documents/${encodeURIComponent(id)}`)).json();
  }

  async original(id: string): Promise<Uint8Array> {
    const response = await this.request(`/v1/documents/${encodeURIComponent(id)}/original`);
    return new Uint8Array(await response.arrayBuffer());
  }
}

async function main() {
  const { values } = parseArgs({ options: {
    server: { type: "string", default: "http://127.0.0.1:8420" },
    url: { type: "string" }, file: { type: "string" },
  } });
  if (Boolean(values.url) === Boolean(values.file)) {
    throw new Error("Supply exactly one of --url URL or --file FILE, with optional --server URL.");
  }
  const client = new Webtool(values.server);
  const document = values.url ? (await client.read(values.url)).document : await client.ingest(values.file!);
  const saved = await client.document(document.id);
  const original = await client.original(saved.id);
  const artifact = saved.source.original;
  if (saved.id !== document.id || original.length !== artifact.size
      || createHash("sha256").update(original).digest("hex") !== artifact.sha256) {
    throw new Error("Saved document or original verification failed");
  }
  console.log(JSON.stringify({ id: saved.id, title: saved.title, blocks: saved.blocks.length,
    warnings: saved.warnings, original_verified: true }));
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((error: unknown) => {
    console.error(error instanceof Error ? error.message : "Client request failed");
    process.exitCode = 1;
  });
}
