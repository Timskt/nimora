import { describe, expect, it } from "vitest";
import { loginLaunchDetail, loginLaunchStatusLabel } from "./StartupSettings";

describe("StartupSettings", () => {
  it("describes authoritative native and inert preview behavior", () => {
    expect(loginLaunchDetail(true)).toContain("系统登录项");
    expect(loginLaunchDetail(true)).toContain("不会自动开启 AI");
    expect(loginLaunchDetail(false)).toContain("不会修改系统登录项");
  });

  it("labels both confirmed states", () => {
    expect(loginLaunchStatusLabel(true)).toBe("登录后自动陪伴");
    expect(loginLaunchStatusLabel(false)).toBe("需要时手动启动");
  });
});
