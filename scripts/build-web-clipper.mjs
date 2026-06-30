#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import {
  createHash,
  createPrivateKey,
  createPublicKey,
  generateKeyPairSync,
  sign as cryptoSign,
  verify as cryptoVerify,
} from "node:crypto";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "..");
const clipperDir = path.join(repoRoot, "web-clipper");
const outDir = path.join(repoRoot, "dist-web-clipper");
const keyPath = path.join(repoRoot, "keepr-web-clipper-selfhost.pem");
const packageJson = JSON.parse(readFileSync(path.join(repoRoot, "package.json"), "utf8"));
const manifest = JSON.parse(readFileSync(path.join(clipperDir, "manifest.json"), "utf8"));

if (manifest.version !== packageJson.version) {
  throw new Error(
    `web-clipper manifest version ${manifest.version} does not match package version ${packageJson.version}`,
  );
}

const requiredEntries = [
  "manifest.json",
  "article-extractor.js",
  "background.js",
  "popup.html",
  "popup.js",
  "options.html",
  "options.js",
  "icons/16.png",
  "icons/32.png",
  "icons/48.png",
  "icons/128.png",
];

for (const rel of requiredEntries) {
  const file = path.join(clipperDir, rel);
  if (!existsSync(file) || !statSync(file).isFile()) {
    throw new Error(`missing web-clipper entry: ${rel}`);
  }
}

rmSync(outDir, { recursive: true, force: true });
mkdirSync(outDir, { recursive: true });

const baseName = `Keepr-Web-Clipper-${packageJson.version}`;
const zipPath = path.join(outDir, `${baseName}.zip`);
const crxPath = path.join(outDir, `${baseName}.crx`);

buildZip(zipPath);
const zipBytes = readFileSync(zipPath);
verifyZip(zipBytes);

const { privateKey, publicKeyDer, generated } = loadOrCreateKey();
const crxBytes = buildCrx(zipBytes, privateKey, publicKeyDer);
writeFileSync(crxPath, crxBytes);
const crxInfo = verifyCrx(readFileSync(crxPath));

const zipHash = sha256Hex(zipBytes);
const crxHash = sha256Hex(crxBytes);

console.log(JSON.stringify({
  version: packageJson.version,
  zip: path.relative(repoRoot, zipPath).replaceAll("\\", "/"),
  zipSha256: zipHash,
  crx: path.relative(repoRoot, crxPath).replaceAll("\\", "/"),
  crxSha256: crxHash,
  extensionId: crxInfo.extensionId,
  generatedKey: generated,
}, null, 2));

function buildZip(destination) {
  const tar = resolveBsdtar();
  const result = spawnSync(tar, ["-a", "-c", "-f", destination, ...requiredEntries], {
    cwd: clipperDir,
    stdio: "inherit",
  });
  if (result.status !== 0) {
    throw new Error(`tar failed with exit code ${result.status}`);
  }
}

function resolveBsdtar() {
  if (process.platform === "win32") {
    const systemRoot = process.env.SystemRoot ?? "C:\\Windows";
    const tar = path.join(systemRoot, "System32", "tar.exe");
    if (existsSync(tar)) return tar;
  }
  return "tar";
}

function verifyZip(bytes) {
  if (bytes.readUInt32LE(0) !== 0x04034b50) {
    throw new Error("zip payload does not start with PK local-file header");
  }
  const entries = listZipEntries(bytes);
  for (const name of entries) {
    if (name.includes("\\")) {
      throw new Error(`zip entry uses a backslash path: ${name}`);
    }
  }
  const names = new Set(entries);
  for (const rel of requiredEntries) {
    if (!names.has(rel)) {
      throw new Error(`zip is missing required entry: ${rel}`);
    }
  }
}

function listZipEntries(bytes) {
  let eocd = -1;
  for (let i = bytes.length - 22; i >= Math.max(0, bytes.length - 65557); i -= 1) {
    if (bytes.readUInt32LE(i) === 0x06054b50) {
      eocd = i;
      break;
    }
  }
  if (eocd < 0) throw new Error("zip EOCD record not found");
  const totalEntries = bytes.readUInt16LE(eocd + 10);
  const centralOffset = bytes.readUInt32LE(eocd + 16);
  const entries = [];
  let offset = centralOffset;
  for (let i = 0; i < totalEntries; i += 1) {
    if (bytes.readUInt32LE(offset) !== 0x02014b50) {
      throw new Error(`bad central-directory header at offset ${offset}`);
    }
    const nameLen = bytes.readUInt16LE(offset + 28);
    const extraLen = bytes.readUInt16LE(offset + 30);
    const commentLen = bytes.readUInt16LE(offset + 32);
    const name = bytes.subarray(offset + 46, offset + 46 + nameLen).toString("utf8");
    entries.push(name.replace(/^\.\//, ""));
    offset += 46 + nameLen + extraLen + commentLen;
  }
  return entries;
}

function loadOrCreateKey() {
  if (existsSync(keyPath)) {
    const privateKey = createPrivateKey(readFileSync(keyPath, "utf8"));
    return {
      privateKey,
      publicKeyDer: createPublicKey(privateKey).export({ type: "spki", format: "der" }),
      generated: false,
    };
  }
  const { privateKey } = generateKeyPairSync("rsa", {
    modulusLength: 2048,
    publicExponent: 0x10001,
  });
  const pem = privateKey.export({ type: "pkcs8", format: "pem" });
  writeFileSync(keyPath, pem, { mode: 0o600 });
  return {
    privateKey,
    publicKeyDer: createPublicKey(privateKey).export({ type: "spki", format: "der" }),
    generated: true,
  };
}

function buildCrx(zipBytes, privateKey, publicKeyDer) {
  const crxId = crxIdBytes(publicKeyDer);
  const signedHeaderData = protoMessage([protoBytes(1, crxId)]);
  const signedPayload = Buffer.concat([
    Buffer.from("CRX3 SignedData\0", "utf8"),
    signedHeaderData,
    zipBytes,
  ]);
  const signature = cryptoSign("sha256", signedPayload, privateKey);
  const keyProof = protoMessage([
    protoBytes(1, publicKeyDer),
    protoBytes(2, signature),
  ]);
  const header = protoMessage([
    protoBytes(2, keyProof),
    protoBytes(10000, signedHeaderData),
  ]);
  const prefix = Buffer.alloc(12);
  prefix.write("Cr24", 0, "ascii");
  prefix.writeUInt32LE(3, 4);
  prefix.writeUInt32LE(header.length, 8);
  return Buffer.concat([prefix, header, zipBytes]);
}

function verifyCrx(bytes) {
  if (bytes.subarray(0, 4).toString("ascii") !== "Cr24") {
    throw new Error("CRX magic mismatch");
  }
  const version = bytes.readUInt32LE(4);
  if (version !== 3) throw new Error(`CRX version ${version} is not CRX3`);
  const headerLen = bytes.readUInt32LE(8);
  const header = bytes.subarray(12, 12 + headerLen);
  const zipBytes = bytes.subarray(12 + headerLen);
  if (zipBytes.readUInt32LE(0) !== 0x04034b50) {
    throw new Error("CRX ZIP payload does not start with PK");
  }
  const headerFields = readProtoFields(header);
  const keyProof = firstField(headerFields, 2);
  const signedHeaderData = firstField(headerFields, 10000);
  const keyFields = readProtoFields(keyProof);
  const publicKeyDer = firstField(keyFields, 1);
  const signature = firstField(keyFields, 2);
  const publicKey = createPublicKey({ key: publicKeyDer, type: "spki", format: "der" });
  const signedPayload = Buffer.concat([
    Buffer.from("CRX3 SignedData\0", "utf8"),
    signedHeaderData,
    zipBytes,
  ]);
  if (!cryptoVerify("sha256", signedPayload, publicKey, signature)) {
    throw new Error("CRX RSA-SHA256 signature verification failed");
  }
  verifyZip(zipBytes);
  return { extensionId: extensionIdFromPublicKey(publicKeyDer) };
}

function protoMessage(fields) {
  return Buffer.concat(fields);
}

function protoBytes(fieldNumber, value) {
  const tag = (BigInt(fieldNumber) << 3n) | 2n;
  return Buffer.concat([varint(tag), varint(BigInt(value.length)), value]);
}

function varint(value) {
  const out = [];
  let n = value;
  while (n >= 0x80n) {
    out.push(Number((n & 0x7fn) | 0x80n));
    n >>= 7n;
  }
  out.push(Number(n));
  return Buffer.from(out);
}

function readProtoFields(bytes) {
  const fields = [];
  let offset = 0;
  while (offset < bytes.length) {
    const tag = readVarint(bytes, offset);
    offset = tag.offset;
    const fieldNumber = Number(tag.value >> 3n);
    const wireType = Number(tag.value & 7n);
    if (wireType !== 2) throw new Error(`unsupported protobuf wire type ${wireType}`);
    const len = readVarint(bytes, offset);
    offset = len.offset;
    const end = offset + Number(len.value);
    fields.push({ fieldNumber, value: bytes.subarray(offset, end) });
    offset = end;
  }
  return fields;
}

function readVarint(bytes, offset) {
  let value = 0n;
  let shift = 0n;
  let i = offset;
  while (i < bytes.length) {
    const byte = bytes[i];
    value |= BigInt(byte & 0x7f) << shift;
    i += 1;
    if ((byte & 0x80) === 0) return { value, offset: i };
    shift += 7n;
  }
  throw new Error("unterminated protobuf varint");
}

function firstField(fields, fieldNumber) {
  const found = fields.find((field) => field.fieldNumber === fieldNumber);
  if (!found) throw new Error(`missing protobuf field ${fieldNumber}`);
  return found.value;
}

function crxIdBytes(publicKeyDer) {
  return createHash("sha256").update(publicKeyDer).digest().subarray(0, 16);
}

function extensionIdFromPublicKey(publicKeyDer) {
  const idBytes = crxIdBytes(publicKeyDer);
  const alphabet = "abcdefghijklmnop";
  let id = "";
  for (const byte of idBytes) {
    id += alphabet[byte >> 4];
    id += alphabet[byte & 0x0f];
  }
  return id;
}

function sha256Hex(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}
