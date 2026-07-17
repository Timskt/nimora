import { describe, expect, it } from "vitest";
import { backupActionDisabled, formatBackupBytes } from "./DataProtection";

describe("DataProtection", () => {
  it("formats backup sizes for compact status rows", () => {
    expect(formatBackupBytes(0)).toBe("1 KB");
    expect(formatBackupBytes(1536)).toBe("2 KB");
    expect(formatBackupBytes(1_572_864)).toBe("1.5 MB");
  });

  it("disables new backups while recovery mode protects the source database", () => {
    expect(backupActionDisabled(true, false)).toBe(true);
    expect(backupActionDisabled(false, true)).toBe(true);
    expect(backupActionDisabled(false, false)).toBe(false);
  });
});
