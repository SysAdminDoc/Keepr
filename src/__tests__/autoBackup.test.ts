import { describe, expect, it } from "vitest";
import { backupFilename, backupPath, cadenceMs, isBackupDue } from "../lib/autoBackup";

const HOUR = 60 * 60 * 1000;

describe("isBackupDue", () => {
  const NOW = Date.UTC(2026, 4, 25, 12, 0, 0);

  it("is false when cadence is off", () => {
    expect(isBackupDue("off", "/path", null, NOW)).toBe(false);
  });

  it("is false when folder is not set", () => {
    expect(isBackupDue("daily", null, null, NOW)).toBe(false);
  });

  it("is true when no backup has ever run", () => {
    expect(isBackupDue("daily", "/path", null, NOW)).toBe(true);
  });

  it("is true once the cadence window has elapsed", () => {
    const lastAt = new Date(NOW - 25 * HOUR).toISOString();
    expect(isBackupDue("daily", "/path", lastAt, NOW)).toBe(true);
  });

  it("is false inside the cadence window", () => {
    const lastAt = new Date(NOW - 12 * HOUR).toISOString();
    expect(isBackupDue("daily", "/path", lastAt, NOW)).toBe(false);
  });

  it("weekly waits 7 days", () => {
    const sixDays = new Date(NOW - 6 * 24 * HOUR).toISOString();
    const sevenDays = new Date(NOW - 7 * 24 * HOUR - 60_000).toISOString();
    expect(isBackupDue("weekly", "/p", sixDays, NOW)).toBe(false);
    expect(isBackupDue("weekly", "/p", sevenDays, NOW)).toBe(true);
  });

  it("malformed timestamp triggers a backup (fail open)", () => {
    expect(isBackupDue("daily", "/p", "not-a-date", NOW)).toBe(true);
  });
});

describe("cadenceMs", () => {
  it("returns Infinity for off", () => {
    expect(cadenceMs("off")).toBe(Infinity);
  });
  it("daily and weekly are 24h / 168h", () => {
    expect(cadenceMs("daily")).toBe(24 * HOUR);
    expect(cadenceMs("weekly")).toBe(7 * 24 * HOUR);
  });
});

describe("backupFilename", () => {
  it("uses an ISO-like prefix that is filename-safe", () => {
    const name = backupFilename(new Date("2026-05-25T12:34:56.789Z"));
    expect(name).toBe("keepr-autobackup-2026-05-25T12-34-56.zip");
  });
});

describe("backupPath", () => {
  it("joins folder + filename with a single forward slash", () => {
    expect(backupPath("/some/folder", "file.zip")).toBe("/some/folder/file.zip");
    expect(backupPath("/some/folder/", "file.zip")).toBe("/some/folder/file.zip");
    expect(backupPath("C:\\backups\\", "file.zip")).toBe("C:\\backups/file.zip");
  });
});
